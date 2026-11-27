import Foundation

/// A relay address as a person types it, checked the way the room protocol takes it: `wss://`
/// for any host, plain `ws://` only for `localhost` and `.local`. A bare host means `wss://`.
public enum RoomRelays {
    /// The address to send, or nil when it isn't one.
    public static func address(_ typed: String) -> String? {
        let text = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, !text.contains(where: \.isWhitespace) else { return nil }
        let full = text.contains("://") ? text : "wss://" + text
        guard let parts = URLComponents(string: full), let scheme = parts.scheme?.lowercased(),
              let host = parts.host?.lowercased(), !host.isEmpty, parts.user == nil
        else { return nil }
        let local = host == "localhost" || host.hasSuffix(".local")
        switch scheme {
        case "wss": guard local || isDomainLike(host) else { return nil }
        case "ws": guard local else { return nil }
        default: return nil
        }
        return full
    }

    /// How the list shows an address: `wss://` goes without saying.
    public static func shown(_ address: String) -> String {
        address.hasPrefix("wss://") ? String(address.dropFirst("wss://".count)) : address
    }

    private static func isDomainLike(_ host: String) -> Bool {
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        return labels.count >= 2 && labels.allSatisfy { !$0.isEmpty }
    }
}
