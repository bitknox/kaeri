// The links a widget or the Live Activity opens the app with, through its kaeri:// scheme.

import Foundation

/// Where a tap on a widget goes.
public enum WidgetLink: Sendable, Equatable {
    /// A title's page: `kaeri://title/900001`.
    case title(Int)
    /// The player, on an episode of a title: `kaeri://play/900001/7`.
    case play(Int, episode: Int)
    /// The Calendar, on today: `kaeri://calendar`.
    case calendar
    /// Settings › Accounts, to connect a tracker: `kaeri://settings/accounts`.
    case accounts
    /// The player, on a finished download: `kaeri://download/42`.
    case download(Int64)

    public static let scheme = "kaeri"

    public var url: URL {
        var components = URLComponents()
        components.scheme = Self.scheme
        switch self {
        case .title(let id):
            components.host = "title"
            components.path = "/\(id)"
        case .play(let id, let episode):
            components.host = "play"
            components.path = "/\(id)/\(episode)"
        case .calendar:
            components.host = "calendar"
        case .accounts:
            components.host = "settings"
            components.path = "/accounts"
        case .download(let id):
            components.host = "download"
            components.path = "/\(id)"
        }
        return components.url!
    }

    /// The link `url` is, when it is one of these.
    public init?(url: URL) {
        guard url.scheme?.lowercased() == Self.scheme, let host = url.host()?.lowercased() else { return nil }
        let parts = url.pathComponents.filter { $0 != "/" }
        switch (host, parts.count) {
        case ("title", 1):
            guard let id = Int(parts[0]), id > 0 else { return nil }
            self = .title(id)
        case ("play", 2):
            guard let id = Int(parts[0]), id > 0, let episode = Int(parts[1]), episode > 0 else { return nil }
            self = .play(id, episode: episode)
        case ("calendar", 0):
            self = .calendar
        case ("settings", 1) where parts[0] == "accounts":
            self = .accounts
        case ("download", 1):
            guard let id = Int64(parts[0]), id > 0 else { return nil }
            self = .download(id)
        default:
            return nil
        }
    }
}
