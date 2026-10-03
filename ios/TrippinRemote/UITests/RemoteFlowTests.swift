import XCTest

/// End-to-end against a real Trippin on the Mac running the Simulator,
/// remote on, PIN in TRIPPIN_PIN (default 4321). The Simulator shares the
/// host's loopback, so the manual address 127.0.0.1 reaches it.
final class RemoteFlowTests: XCTestCase {
    var app: XCUIApplication!
    let host = ProcessInfo.processInfo.environment["TRIPPIN_HOST"] ?? "127.0.0.1"
    let pin = ProcessInfo.processInfo.environment["TRIPPIN_PIN"] ?? "4321"

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    // MARK: helpers

    func shot(_ name: String) {
        let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        a.name = name
        a.lifetime = .keepAlways
        add(a)
    }

    @discardableResult
    func wait(_ el: XCUIElement, value contains: String? = nil, timeout: TimeInterval = 10,
              _ what: String) -> XCUIElement {
        let pred: NSPredicate
        if let contains {
            pred = NSPredicate(format: "exists == true AND value CONTAINS %@", contains)
        } else {
            pred = NSPredicate(format: "exists == true")
        }
        let r = XCTWaiter().wait(for: [XCTNSPredicateExpectation(predicate: pred, object: el)], timeout: timeout)
        if r != .completed {
            shot("FAILED \(what)")
            XCTFail("timed out: \(what) (value \(el.exists ? String(describing: el.value) : "missing"))")
        }
        return el
    }

    var isPhone: Bool { app.tabBars.firstMatch.exists }

    /// iPhone: tab bar; iPad: the segmented page picker (Scenes is always on
    /// screen). Taps until the page's marker shows — a segment tap can be
    /// swallowed right after a rotation.
    func page(_ name: String) {
        let marker = app.descendants(matching: .any)["page.\(name.lowercased())"]
        for _ in 0..<3 {
            if isPhone {
                app.tabBars.buttons[name].tap()
            } else if name != "Scenes" {
                app.segmentedControls.buttons[name].tap()
            }
            if name == "Scenes" || marker.waitForExistence(timeout: 2) { return }
        }
        XCTFail("page \(name) didn't show")
    }

    func connectManually(pin typed: String?) {
        let hostField = wait(app.textFields["host"], timeout: 15, "connect screen")
        hostField.tap()
        // The last address is remembered: clear it before typing.
        if let old = hostField.value as? String, !old.isEmpty, old != hostField.placeholderValue {
            hostField.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count))
        }
        hostField.typeText(host)
        app.buttons["connect"].tap()
        if let typed {
            let pinField = wait(app.textFields["pin"], "pin field")
            pinField.tap()
            pinField.typeText(typed)
            app.buttons["pinConnect"].tap()
        }
    }

    // MARK: tests

    func test1_PinSceneThumbPaletteStrobe() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        shot("01 connect")

        // A wrong PIN is refused, and the app stops retrying.
        connectManually(pin: "0000")
        wait(app.descendants(matching: .any)["badPin"], "wrong PIN banner")
        shot("02 wrong pin")

        // The PIN field comes back; the right one connects.
        let pinField = wait(app.textFields["pin"], "pin field again")
        pinField.tap()
        pinField.typeText(pin)
        app.buttons["pinConnect"].tap()
        let link = app.descendants(matching: .any)["link"]
        wait(link, value: "Live", timeout: 10, "connected")
        sleep(1)
        shot("03 perform")

        // Strobe pad: lights when the server echoes strobe:true.
        let strobe = wait(app.buttons["pad.Strobe"], "strobe pad")
        strobe.tap()
        wait(strobe, value: "on", timeout: 3, "strobe on echoed")
        shot("04 strobe on")
        strobe.tap()
        wait(strobe, value: "off", timeout: 3, "strobe off echoed")

        // Scenes: thumbnails load; tap cuts; hold queues next.
        page("Scenes")
        let first = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'scene.'")).firstMatch
        wait(first, value: "thumb", timeout: 20, "first thumbnail")
        let target = app.descendants(matching: .any)["scene.aurora"]
        wait(target, "aurora tile")
        target.tap()
        wait(target, value: "live", timeout: 3, "aurora live")
        let queued = app.descendants(matching: .any)["scene.arch_run"]
        queued.press(forDuration: 0.6)
        wait(queued, value: "next", timeout: 3, "arch_run queued next")
        sleep(2) // let more thumbnails land for the screenshot
        shot("05 scenes")

        // Look: pick a palette that isn't the current one.
        page("Look")
        let chips = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'palette.'"))
        wait(chips.firstMatch, "palette chips")
        let pick = (0..<chips.count).map { chips.element(boundBy: $0) }
            .first { ($0.value as? String) != "selected" }!
        pick.tap()
        wait(pick, value: "selected", timeout: 3, "palette \(pick.identifier) selected")
        shot("06 look")

        page("Dancer")
        sleep(1)
        shot("07 dancer")
        page("Timeline")
        sleep(1)
        shot("08 timeline")
    }

    /// Run while the host kills and restarts Trippin: the link must drop to
    /// "Lost" and come back "Live" by itself (PIN remembered from test1).
    func test2_ReconnectAfterRestart() {
        app.launch()
        connectManually(pin: nil) // saved PIN connects straight away
        let link = app.descendants(matching: .any)["link"]
        wait(link, value: "Live", timeout: 10, "connected")
        shot("10 live before restart")
        wait(link, value: "Lost", timeout: 60, "link lost when Trippin quits")
        shot("11 reconnecting")
        wait(link, value: "Live", timeout: 60, "link back after Trippin restarts")
        sleep(1)
        shot("12 live after restart")
    }

    /// Bonjour: "Trippin on <host>" shows up on the connect screen and
    /// connects with the PIN. Needs a Trippin whose mDNS advert reaches the
    /// host's mDNSResponder (macOS Local Network permission for whatever
    /// launched it).
    func test3_Discovery() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        // TRIPPIN_SERVICE picks one when several Trippins advertise.
        let wanted = ProcessInfo.processInfo.environment["TRIPPIN_SERVICE"]
        let server = wanted.map { app.buttons["server.\($0)"] }
            ?? app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH 'server.'")).firstMatch
        wait(server, timeout: 20, "Bonjour server listed")
        shot("20 discovered")
        server.tap()
        let pinField = wait(app.textFields["pin"], "pin field")
        pinField.tap()
        pinField.typeText(pin)
        app.buttons["pinConnect"].tap()
        wait(app.descendants(matching: .any)["link"], value: "Live", timeout: 10, "connected via Bonjour")
        shot("21 live via bonjour")
    }

    // MARK: no server needed

    /// Every page in the built-in demo, portrait then landscape — the
    /// layout check, no Trippin required.
    func test4_DemoTour() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        app.buttons["demo"].tap()
        let link = app.descendants(matching: .any)["link"]
        wait(link, value: "Live", timeout: 5, "demo connected")
        for (orientation, tag) in [(UIDeviceOrientation.portrait, "portrait"), (.landscapeLeft, "landscape")] {
            XCUIDevice.shared.orientation = orientation
            sleep(1)
            for name in ["Perform", "Scenes", "Dancer", "Look", "Timeline"] {
                if !isPhone && name == "Scenes" { continue } // always on screen on iPad
                page(name)
                if name == "Scenes" || (!isPhone && name == "Perform") {
                    // Let the thumbnails land.
                    let first = app.descendants(matching: .any).matching(NSPredicate(format: "identifier BEGINSWITH 'scene.'")).firstMatch
                    wait(first, value: "thumb", timeout: 10, "demo thumbnail")
                }
                sleep(1)
                shot("30 \(tag) \(name.lowercased())")
            }
        }
        XCUIDevice.shared.orientation = .portrait
        // Demo commands work like the real server's.
        page("Perform")
        let strobe = app.buttons["pad.Strobe"]
        strobe.tap()
        wait(strobe, value: "on", timeout: 2, "demo strobe on")
    }

    /// A saved server can be forgotten from the connect screen.
    func test5_ForgetSaved() {
        app.launchArguments = ["-uitestReset", "-uitestSeedSaved"]
        app.launch()
        let row = wait(app.buttons["saved.192.168.0.50:9138"], timeout: 10, "saved row")
        shot("40 saved")
        app.buttons["options.192.168.0.50:9138"].tap()
        wait(app.buttons["Forget"], "forget menu item").tap()
        shot("41 confirm forget")
        // The confirmation dialog's destructive button.
        let confirm = app.sheets.buttons["Forget"].exists ? app.sheets.buttons["Forget"] : app.buttons["Forget"].firstMatch
        confirm.tap()
        let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: row)
        XCTAssertEqual(XCTWaiter().wait(for: [gone], timeout: 3), .completed, "saved row removed")
        shot("42 forgotten")
    }

    /// Every toggle pad lights when its state goes on and goes dark when it
    /// goes off again (demo server, which flips the same fields main.rs does).
    func test6_TogglePadsLight() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        app.buttons["demo"].tap()
        wait(app.descendants(matching: .any)["link"], value: "Live", timeout: 5, "demo connected")
        page("Perform")
        let toggles = ["Strobe", "Blackout", "ToggleDancer", "ToggleRandom", "RecordSet",
                       "ToggleLogo", "ToggleName", "ToggleTicker", "Fullscreen"]
        for key in toggles {
            let pad = app.buttons["pad.\(key)"]
            if !pad.exists { app.swipeUp() }
            wait(pad, timeout: 3, "\(key) pad")
            let before = (pad.value as? String) ?? ""
            let flipped = before == "on" ? "off" : "on"
            pad.tap()
            wait(pad, value: flipped, timeout: 2, "\(key) lit state flips to \(flipped)")
            pad.tap()
            wait(pad, value: before, timeout: 2, "\(key) lit state back to \(before)")
        }
        // Radio group: exactly the chosen mode lights.
        app.swipeDown(); app.swipeDown()
        app.buttons["pad.ModeManual"].tap()
        wait(app.buttons["pad.ModeManual"], value: "on", timeout: 2, "Manual lit")
        wait(app.buttons["pad.ModeAuto"], value: "off", timeout: 2, "Auto unlit")
        shot("50 toggles")
    }

    // MARK: App Store material

    /// Staged screens for the App Store listing, from the demo. iPhone in
    /// portrait; iPad in landscape (the split view is its best side) plus
    /// one portrait. Run with the status bar overridden (9:41, full).
    func test7_StoreShots() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        app.buttons["demo"].tap()
        wait(app.descendants(matching: .any)["link"], value: "Live", timeout: 5, "demo connected")
        let iPad = !isPhone
        if iPad { XCUIDevice.shared.orientation = .landscapeLeft; sleep(1) }

        // Something queued, strobe lit: a live-looking show.
        page("Scenes")
        let next = app.descendants(matching: .any)["scene.synthwave"]
        if !next.exists { app.swipeUp() }
        wait(next, timeout: 5, "synthwave tile")
        next.press(forDuration: 0.6)
        wait(next, value: "next", timeout: 3, "synthwave queued")
        if !iPad { app.swipeDown() }
        let first = app.descendants(matching: .any)["scene.laser_show"]
        wait(first, value: "thumb", timeout: 10, "thumbnails loaded")
        sleep(2)
        if isPhone { shot("store-02-scenes") }

        page("Perform")
        let strobe = wait(app.buttons["pad.Strobe"], "strobe pad")
        strobe.tap()
        wait(strobe, value: "on", timeout: 2, "strobe on")
        sleep(1)
        shot("store-01-perform")

        page("Look")
        sleep(1)
        shot("store-03-look")

        page("Dancer")
        sleep(1)
        shot("store-04-dancer")

        page("Timeline")
        let play = app.buttons.matching(NSPredicate(format: "label == 'Play'")).firstMatch
        if play.exists { play.tap() }
        sleep(3)
        shot("store-05-timeline")

        if iPad {
            XCUIDevice.shared.orientation = .portrait
            sleep(1)
            page("Perform")
            sleep(1)
            shot("store-06-portrait")
        }
        XCUIDevice.shared.orientation = .portrait
    }

    /// A paced tour for the App Store preview video: the host screen-records
    /// the Simulator while this runs, then trims from the first Perform frame.
    /// TRIPPIN_PREVIEW_LANDSCAPE=1 runs it in landscape (iPad).
    func test8_PreviewTour() {
        app.launchArguments = ["-uitestReset"]
        app.launch()
        if ProcessInfo.processInfo.environment["TRIPPIN_PREVIEW_LANDSCAPE"] == "1" {
            XCUIDevice.shared.orientation = .landscapeLeft
        }
        app.buttons["demo"].tap()
        wait(app.descendants(matching: .any)["link"], value: "Live", timeout: 5, "demo connected")
        let beat: UInt32 = 1_200_000 // µs between moves: a calm, readable pace
        page("Perform")
        usleep(beat * 2)
        app.buttons["pad.Strobe"].tap(); usleep(beat * 2)
        app.buttons["pad.Strobe"].tap(); usleep(beat)
        app.buttons["pad.NextScene"].tap(); usleep(beat * 2)

        page("Scenes")
        for name in ["synthwave", "laser_show", "ocean"] {
            let tile = app.descendants(matching: .any)["scene.\(name)"]
            if tile.waitForExistence(timeout: 2) { tile.tap() }
            usleep(beat * 2)
        }
        let q = app.descendants(matching: .any)["scene.stage_rig"]
        if q.waitForExistence(timeout: 2) { q.press(forDuration: 0.6) }
        usleep(beat * 2)

        page("Look")
        usleep(beat)
        for p in ["sunset", "cyber", "ocean"] {
            let chip = app.buttons["palette.\(p)"]
            if chip.waitForExistence(timeout: 2) { chip.tap() }
            usleep(beat)
        }
        let kaleido = app.buttons["Kaleido6"]
        if kaleido.exists { kaleido.tap(); usleep(beat * 2) }

        page("Dancer")
        usleep(beat)
        for c in ["house shuffle", "vogue"] {
            let b = app.buttons[c]
            if b.waitForExistence(timeout: 2) { b.tap() }
            usleep(beat * 2)
        }

        page("Timeline")
        let play = app.buttons.matching(NSPredicate(format: "label == 'Play'")).firstMatch
        if play.exists { play.tap() }
        usleep(beat * 3)
        page("Perform")
        usleep(beat * 2)
        XCUIDevice.shared.orientation = .portrait
    }
}
