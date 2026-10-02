import Foundation
import Network
import UIKit

/// Where to connect: a discovered Bonjour service or a typed address.
enum Target: Hashable {
    case bonjour(name: String, endpoint: NWEndpoint)
    case manual(host: String, port: UInt16)
    /// The in-app stand-in server (`DemoServer`).
    case demo

    /// Keychain account + display name.
    var key: String {
        switch self {
        case .bonjour(let name, _): return name
        case .manual(let host, let port): return "\(host):\(port)"
        case .demo: return "Demo"
        }
    }
}

enum LinkStatus: Equatable {
    case idle
    case connecting
    case connected
    /// Dropped; retrying after `retryIn` seconds.
    case lost(reason: String, retryIn: Double)
    /// The server refused the PIN — no auto-retry until a new one is typed.
    case badPin
}

/// The WebSocket link to one Trippin: hello/PIN, state frames, commands,
/// thumbnails, and reconnect with backoff.
@MainActor
final class Connection: ObservableObject {
    @Published private(set) var status: LinkStatus = .idle
    @Published private(set) var info = ServerInfo()
    @Published private(set) var state = ShowState()
    @Published private(set) var target: Target?
    /// The last `err` frame, shown briefly.
    @Published var lastError: String?
    /// This target has said hello at least once — the control UI stays up
    /// (with a "reconnecting" banner) through drops after that.
    @Published private(set) var hasSession = false
    let thumbs = ThumbCache()

    private var pin = ""
    private var task: URLSessionWebSocketTask?
    private let session = URLSession(configuration: .default)
    /// Bumped per attempt so callbacks from a dead socket are ignored.
    private var gen = 0
    private var attempt = 0
    private var helloDone = false
    private var lastFrame = Date.distantPast
    private var retry: Task<Void, Never>?
    private var watchdog: Timer?
    private var demo: DemoServer?

    var isConnected: Bool { status == .connected }

    init() {
        thumbs.send = { [weak self] scene in self?.send(["cmd": "thumb", "scene": scene]) }
        // Coming back to the foreground: reconnect now rather than waiting
        // out the backoff (iOS kills sockets in the background).
        NotificationCenter.default.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.wake() }
        }
    }

    // MARK: lifecycle

    func connect(_ t: Target, pin: String) {
        disconnect()
        target = t
        self.pin = pin
        attempt = 0
        if t == .demo {
            let d = DemoServer { [weak self] text in self?.handle(text) }
            demo = d
            d.start()
        } else {
            open()
        }
    }

    /// User-initiated: stop and forget the target.
    func disconnect() {
        retry?.cancel(); retry = nil
        watchdog?.invalidate(); watchdog = nil
        demo?.stop(); demo = nil
        gen += 1
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        target = nil
        helloDone = false
        hasSession = false
        status = .idle
    }

    private func wake() {
        guard target != nil, demo == nil else { return }
        if case .lost = status {
            retry?.cancel()
            attempt = 0
            open()
        } else if status == .connected, Date().timeIntervalSince(lastFrame) > 2 {
            dropped("no data")
        }
    }

    private func open() {
        guard let t = target else { return }
        gen += 1
        let my = gen
        helloDone = false
        status = .connecting
        Task {
            let hostPort: (String, UInt16)
            do {
                switch t {
                case .manual(let h, let p): hostPort = (h, p)
                case .demo: return
                case .bonjour(_, let ep): hostPort = try await resolve(ep)
                }
            } catch {
                guard my == gen else { return }
                return dropped("can't find \(t.key)")
            }
            guard my == gen else { return }
            let host = hostPort.0.contains(":") ? "[\(hostPort.0)]" : hostPort.0
            guard let url = URL(string: "ws://\(host):\(hostPort.1)/") else { return dropped("bad address") }
            var req = URLRequest(url: url)
            req.timeoutInterval = 5
            let ws = session.webSocketTask(with: req)
            ws.maximumMessageSize = 16 << 20 // thumbnails are base64 PNGs
            task = ws
            ws.resume()
            send(["cmd": "hello", "v": 1, "name": UIDevice.current.name, "pin": pin])
            receive(ws, my)
            startWatchdog(my)
        }
    }

    private func receive(_ ws: URLSessionWebSocketTask, _ my: Int) {
        ws.receive { [weak self] result in
            Task { @MainActor in
                guard let self, my == self.gen else { return }
                switch result {
                case .failure(let e):
                    self.dropped(self.helloDone ? "connection lost" : Self.describe(e))
                case .success(let msg):
                    switch msg {
                    case .string(let s): self.handle(s)
                    case .data(let d): self.handle(String(decoding: d, as: UTF8.self))
                    @unknown default: break
                    }
                    if my == self.gen { self.receive(ws, my) }
                }
            }
        }
    }

    private static func describe(_ e: Error) -> String {
        let ns = e as NSError
        if ns.domain == NSURLErrorDomain {
            switch ns.code {
            case NSURLErrorTimedOut: return "timed out"
            case NSURLErrorCannotConnectToHost: return "refused — is the remote on in Trippin?"
            case NSURLErrorNotConnectedToInternet, NSURLErrorNetworkConnectionLost: return "no network"
            default: break
            }
        }
        return e.localizedDescription
    }

    /// The server pushes ~10 Hz; 3 s of silence means the link is dead
    /// even if TCP hasn't noticed (Wi-Fi roam, laptop asleep).
    private func startWatchdog(_ my: Int) {
        watchdog?.invalidate()
        lastFrame = Date()
        watchdog = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in
                guard let self, my == self.gen else { return }
                if Date().timeIntervalSince(self.lastFrame) > 3 {
                    self.dropped(self.helloDone ? "no data from Trippin" : "no answer")
                }
            }
        }
    }

    private func dropped(_ reason: String) {
        gen += 1
        task?.cancel(with: .abnormalClosure, reason: nil)
        task = nil
        watchdog?.invalidate(); watchdog = nil
        helloDone = false
        thumbs.linkDropped()
        guard target != nil, status != .badPin else { return }
        // 0.5, 1, 2, 4, 8, then every 10 s.
        let delay = min(10, 0.5 * pow(2, Double(min(attempt, 5))))
        attempt += 1
        status = .lost(reason: reason, retryIn: delay)
        retry?.cancel()
        retry = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1e9))
            guard !Task.isCancelled else { return }
            self?.open()
        }
    }

    // MARK: frames

    private func handle(_ text: String) {
        lastFrame = Date()
        guard let j = JSON.parse(text) else { return }
        switch j["type"].string {
        case "hello":
            info = ServerInfo(j)
            helloDone = true
            hasSession = true
            attempt = 0
            status = .connected
            lastError = nil
            if let t = target, t != .demo {
                PinStore.set(pin, for: t.key)
                Saved.remember(t)
            }
            thumbs.reset(for: info.version + "|" + info.scenes.joined(separator: ","))
        case "state":
            state = ShowState(j)
        case "thumb":
            if let scene = j["scene"].string, let b64 = j["png_b64"].string {
                thumbs.received(scene, b64)
            }
        case "err":
            let msg = j["msg"].string ?? "error"
            if !helloDone, msg.localizedCaseInsensitiveContains("pin") {
                // Rejected: stop, don't hammer the server (it penalises).
                status = .badPin
                hasSession = false
                gen += 1
                retry?.cancel()
                watchdog?.invalidate()
                task?.cancel(with: .normalClosure, reason: nil)
                task = nil
                if let t = target { PinStore.remove(t.key) }
            } else {
                lastError = msg
            }
        default: break
        }
    }

    // MARK: commands

    func send(_ obj: [String: Any]) {
        if let demo { return demo.receive(obj) }
        guard let task, let data = try? JSONSerialization.data(withJSONObject: obj) else { return }
        task.send(.string(String(decoding: data, as: UTF8.self))) { _ in }
    }

    func act(_ a: RemoteAction) { send(["cmd": "action", "action": a.id.any]) }
    func act(_ key: String) {
        if let a = info.action(key) { act(a) } else { send(["cmd": "action", "action": key]) }
    }
    func goTo(_ scene: String) { send(["cmd": "goto_scene", "scene": scene]) }
    func queueNext(_ scene: String) { send(["cmd": "queue_next", "scene": scene]) }
    func showClip(_ clip: String) { send(["cmd": "show_clip", "clip": clip]) }
    func set(_ key: String, _ value: Any) { send(["cmd": "set", "key": key, "value": value]) }
    func transport(_ op: String, pos: Double? = nil) {
        var m: [String: Any] = ["cmd": "transport", "op": op]
        if let pos { m["pos"] = pos }
        send(m)
    }
}

/// Scene thumbnails: requested as tiles appear, dripped out so the render
/// thread isn't asked for 130 at once, cached for the session.
@MainActor
final class ThumbCache: ObservableObject {
    @Published private(set) var images: [String: UIImage] = [:]
    var send: ((String) -> Void)?
    private var asked: Set<String> = []
    private var queue: [String] = []
    private var drip: Task<Void, Never>?
    private var generation = ""

    func image(_ scene: String) -> UIImage? { images[scene] }

    func want(_ scene: String) {
        guard images[scene] == nil, !asked.contains(scene) else { return }
        asked.insert(scene)
        queue.append(scene)
        startDrip()
    }

    private func startDrip() {
        guard drip == nil else { return }
        drip = Task { [weak self] in
            while let self, !Task.isCancelled, !self.queue.isEmpty {
                let s = self.queue.removeFirst()
                self.send?(s)
                try? await Task.sleep(nanoseconds: 40_000_000)
            }
            self?.drip = nil
        }
    }

    func received(_ scene: String, _ b64: String) {
        guard let data = Data(base64Encoded: b64), let img = UIImage(data: data) else { return }
        images[scene] = img
    }

    /// A different Trippin (or a changed scene list) invalidates the cache.
    func reset(for generation: String) {
        if generation != self.generation {
            images = [:]
            self.generation = generation
        }
        asked = Set(images.keys)
    }

    /// Unanswered requests die with the socket; ask again after reconnect.
    func linkDropped() {
        drip?.cancel(); drip = nil
        queue.removeAll()
        asked = Set(images.keys)
    }
}
