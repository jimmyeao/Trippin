import Foundation
import Network
import UIKit

/// A Trippin advertising `_trippin._tcp` ("Trippin on <host>").
struct FoundServer: Identifiable, Hashable {
    let name: String
    let endpoint: NWEndpoint
    var id: String { name }
}

/// Browses Bonjour for Trippin instances while the connect screen is up.
///
/// The browse session dies whenever mDNSResponder drops it — app suspended,
/// phone asleep, Wi-Fi change (DNSServiceErr -65569, DefunctConnection) —
/// and a failed NWBrowser never recovers, so it's rebuilt: after a failure
/// (with backoff) and on every return to the foreground. Only a real policy
/// denial (-65570: Local Network permission off) is shown to the user.
@MainActor
final class Discovery: ObservableObject {
    @Published private(set) var servers: [FoundServer] = []
    @Published private(set) var error: String?
    private var browser: NWBrowser?
    /// Wanted by the UI (between start() and stop()).
    private var active = false
    private var failures = 0
    private var restart: Task<Void, Never>?
    private var foreground: NSObjectProtocol?

    private static let policyDenied: Int32 = -65570

    func start() {
        active = true
        if foreground == nil {
            foreground = NotificationCenter.default.addObserver(
                forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main
            ) { [weak self] _ in
                Task { @MainActor in self?.rebuild() }
            }
        }
        if browser == nil { open() }
    }

    func stop() {
        active = false
        restart?.cancel(); restart = nil
        browser?.cancel()
        browser = nil
        if let foreground { NotificationCenter.default.removeObserver(foreground) }
        foreground = nil
    }

    /// Tear down and browse again now (foregrounding: the old session is
    /// likely defunct even if it hasn't reported it yet).
    private func rebuild() {
        guard active else { return }
        restart?.cancel(); restart = nil
        browser?.cancel()
        browser = nil
        failures = 0
        open()
    }

    private func open() {
        let b = NWBrowser(for: .bonjour(type: "_trippin._tcp", domain: nil), using: .tcp)
        b.browseResultsChangedHandler = { [weak self] results, _ in
            // One result per interface the advert was seen on (Wi-Fi,
            // Ethernet, a VPN adapter…): keep one row per service name —
            // the endpoint is resolved by name on connect anyway.
            var byName: [String: FoundServer] = [:]
            for r in results {
                guard case let .service(name, _, _, _) = r.endpoint, byName[name] == nil else { continue }
                byName[name] = FoundServer(name: name, endpoint: r.endpoint)
            }
            let found = byName.values.sorted { $0.name < $1.name }
            Task { @MainActor in self?.servers = found }
        }
        b.stateUpdateHandler = { [weak self, weak b] state in
            Task { @MainActor in
                guard let self, let b, b === self.browser else { return }
                switch state {
                case .ready:
                    self.error = nil
                    self.failures = 0
                case .waiting(let e):
                    // Waiting recovers by itself (network back, permission
                    // granted); only a denial needs the user.
                    self.error = Self.isDenied(e) ? Self.deniedText : nil
                case .failed(let e):
                    self.error = Self.isDenied(e) ? Self.deniedText : nil
                    self.scheduleRestart()
                default: break
                }
            }
        }
        b.start(queue: .main)
        browser = b
    }

    /// Rebuild after 1, 2, 4… up to 15 s.
    private func scheduleRestart() {
        browser?.cancel()
        browser = nil
        guard active else { return }
        let delay = min(15, pow(2, Double(min(failures, 4))))
        failures += 1
        restart?.cancel()
        restart = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1e9))
            guard let self, !Task.isCancelled, self.active, self.browser == nil else { return }
            self.open()
        }
    }

    private static let deniedText = "Trippin Remote isn't allowed on the local network. Turn it on in Settings → Privacy & Security → Local Network."

    private static func isDenied(_ e: NWError) -> Bool {
        if case .dns(let code) = e { return code == policyDenied }
        return false
    }
}

/// Resolve a Bonjour service to an IPv4 host:port by opening (and
/// immediately dropping) a TCP connection — URLSession's WebSocket wants a
/// URL, and the address can change between reconnects, so this runs on
/// every connect attempt.
func resolve(_ endpoint: NWEndpoint, timeout: TimeInterval = 4) async throws -> (String, UInt16) {
    let params = NWParameters.tcp
    if let ip = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
        ip.version = .v4
    }
    let conn = NWConnection(to: endpoint, using: params)
    return try await withCheckedThrowingContinuation { cont in
        var done = false
        let finish: (Result<(String, UInt16), Error>) -> Void = { r in
            guard !done else { return }
            done = true
            conn.cancel()
            cont.resume(with: r)
        }
        conn.stateUpdateHandler = { state in
            switch state {
            case .ready:
                if case let .hostPort(host, port)? = conn.currentPath?.remoteEndpoint {
                    var h = "\(host)"
                    if let pct = h.firstIndex(of: "%") { h = String(h[..<pct]) } // drop an interface scope
                    finish(.success((h, port.rawValue)))
                } else {
                    finish(.failure(URLError(.cannotFindHost)))
                }
            case .failed(let e): finish(.failure(e))
            case .waiting(let e): finish(.failure(e))
            default: break
            }
        }
        conn.start(queue: .main)
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout) {
            finish(.failure(URLError(.timedOut)))
        }
    }
}
