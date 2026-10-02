import Foundation
import Network

/// A Trippin advertising `_trippin._tcp` ("Trippin on <host>").
struct FoundServer: Identifiable, Hashable {
    let name: String
    let endpoint: NWEndpoint
    var id: String { name }
}

/// Browses Bonjour for Trippin instances while the connect screen is up.
@MainActor
final class Discovery: ObservableObject {
    @Published private(set) var servers: [FoundServer] = []
    @Published private(set) var error: String?
    private var browser: NWBrowser?

    func start() {
        guard browser == nil else { return }
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
        b.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                switch state {
                case .failed(let e), .waiting(let e):
                    // .waiting with a policy denial = Local Network permission off.
                    self?.error = "Can't browse the network (\(e.localizedDescription)). Check Settings → Privacy → Local Network."
                case .ready: self?.error = nil
                default: break
                }
            }
        }
        b.start(queue: .main)
        browser = b
    }

    func stop() {
        browser?.cancel()
        browser = nil
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
