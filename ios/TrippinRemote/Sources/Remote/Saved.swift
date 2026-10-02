import Foundation
import Security

/// Servers this device has connected to, for the connect screen's Saved
/// list. The address list lives in UserDefaults; each server's PIN stays in
/// the Keychain (`PinStore`). Forgetting a server removes both.
struct SavedServer: Codable, Hashable, Identifiable {
    enum Kind: String, Codable { case bonjour, manual }
    var kind: Kind
    /// Bonjour service name, or "host:port" — the same key `PinStore` uses.
    var key: String
    var host: String?
    var port: UInt16?
    var lastUsed: Date
    var id: String { key }
}

@MainActor
enum Saved {
    private static let defaultsKey = "savedServers"

    static func all() -> [SavedServer] {
        var list: [SavedServer] = []
        if let data = UserDefaults.standard.data(forKey: defaultsKey),
           let decoded = try? JSONDecoder().decode([SavedServer].self, from: data) {
            list = decoded
        }
        // PINs saved before this list existed still count as saved servers.
        for account in PinStore.accounts() where !list.contains(where: { $0.key == account }) {
            list.append(entry(forKey: account))
        }
        return list.sorted { $0.lastUsed > $1.lastUsed }
    }

    static func remember(_ t: Target) {
        var list = all().filter { $0.key != t.key }
        switch t {
        case .bonjour(let name, _):
            list.append(SavedServer(kind: .bonjour, key: name, lastUsed: Date()))
        case .manual(let host, let port):
            list.append(SavedServer(kind: .manual, key: t.key, host: host, port: port, lastUsed: Date()))
        case .demo:
            return
        }
        store(list)
    }

    /// Remove the server and its PIN.
    static func forget(_ key: String) {
        store(all().filter { $0.key != key })
        PinStore.remove(key)
    }

    static func forgetAll() {
        UserDefaults.standard.removeObject(forKey: defaultsKey)
        PinStore.removeAll()
    }

    private static func store(_ list: [SavedServer]) {
        if let data = try? JSONEncoder().encode(list) {
            UserDefaults.standard.set(data, forKey: defaultsKey)
        }
    }

    /// A Keychain account with no list entry: "host:port" is a typed
    /// address, anything else a Bonjour name.
    private static func entry(forKey key: String) -> SavedServer {
        if let colon = key.lastIndex(of: ":"), let port = UInt16(key[key.index(after: colon)...]) {
            return SavedServer(kind: .manual, key: key, host: String(key[..<colon]), port: port, lastUsed: .distantPast)
        }
        return SavedServer(kind: .bonjour, key: key, lastUsed: .distantPast)
    }
}
