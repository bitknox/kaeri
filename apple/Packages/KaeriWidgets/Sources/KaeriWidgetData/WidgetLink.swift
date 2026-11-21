// The links a widget or the Live Activity opens the app with, through its kaeri:// scheme. Any
// page or message can open these too, so none of them plays from the network: an episode plays
// only through the widget's play intent, which runs in the app.

import Foundation

/// Where a tap on a widget goes.
public enum WidgetLink: Sendable, Equatable {
    /// A title's page: `kaeri://title/900001`.
    case title(Int)
    /// The player, on an episode of a title, from the widget's play intent only. As a link it is
    /// its title's page.
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
        case .title(let id), .play(let id, _):
            components.host = "title"
            components.path = "/\(id)"
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
            // The widgets' play links before the intent: whoever made one, the viewer presses Play.
            guard let id = Int(parts[0]), id > 0, let episode = Int(parts[1]), episode > 0 else { return nil }
            self = .title(id)
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
