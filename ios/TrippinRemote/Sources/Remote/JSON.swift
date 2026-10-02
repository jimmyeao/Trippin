import Foundation

/// A loosely typed JSON value. The server's frames are read field by field
/// so a new or retyped field never breaks decoding of the rest; action ids
/// are echoed back verbatim, whatever shape serde gave them.
enum JSON: Equatable, Hashable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    init(_ any: Any?) {
        switch any {
        case nil, is NSNull: self = .null
        case let n as NSNumber:
            // NSNumber bridges both Bool and numbers; tell them apart.
            if CFGetTypeID(n) == CFBooleanGetTypeID() { self = .bool(n.boolValue) } else { self = .number(n.doubleValue) }
        case let s as String: self = .string(s)
        case let a as [Any]: self = .array(a.map { JSON($0) })
        case let d as [String: Any]: self = .object(d.mapValues { JSON($0) })
        default: self = .null
        }
    }

    static func parse(_ text: String) -> JSON? {
        guard let data = text.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        else { return nil }
        return JSON(obj)
    }

    var any: Any {
        switch self {
        case .null: return NSNull()
        case .bool(let b): return b
        case .number(let n): return n
        case .string(let s): return s
        case .array(let a): return a.map(\.any)
        case .object(let o): return o.mapValues(\.any)
        }
    }

    func encoded() -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: any, options: [.fragmentsAllowed]) else { return "null" }
        return String(decoding: data, as: UTF8.self)
    }

    subscript(key: String) -> JSON {
        if case .object(let o) = self { return o[key] ?? .null }
        return .null
    }

    var string: String? {
        switch self {
        case .string(let s): return s
        case .number(let n): return n == n.rounded() ? String(Int(n)) : String(n)
        default: return nil
        }
    }
    var double: Double? { if case .number(let n) = self { return n }; return nil }
    var int: Int? { double.map { Int($0) } }
    var bool: Bool? { if case .bool(let b) = self { return b }; return nil }
    var array: [JSON] { if case .array(let a) = self { return a }; return [] }
    var isNull: Bool { self == .null }

    /// A short human label: strings as-is, an externally tagged enum
    /// (`{"Scene": 3}`) as its tag, anything else as compact JSON.
    var label: String {
        switch self {
        case .string(let s): return s
        case .object(let o) where o.count == 1: return o.keys.first!
        default: return encoded()
        }
    }
}
