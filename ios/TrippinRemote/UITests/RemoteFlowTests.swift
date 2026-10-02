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

    /// iPhone: tab bar; iPad: the segmented page picker (Scenes is always on screen).
    func page(_ name: String) {
        if isPhone {
            app.tabBars.buttons[name].tap()
        } else if name != "Scenes" {
            app.segmentedControls.buttons[name].tap()
        }
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
}
