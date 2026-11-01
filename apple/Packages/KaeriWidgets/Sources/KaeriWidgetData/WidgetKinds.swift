// The widgets' kinds, as WidgetKit names them: what the app looks for among the widgets placed,
// since it keeps its background refresh while any of them is.

/// The kinds of Kaeri's widgets.
public enum WidgetKinds {
    /// Continue watching.
    public static let continueWatching = "app.kaeri.continue-watching"
    /// Airing today.
    public static let airingToday = "app.kaeri.airing-today"
    /// Both.
    public static let all: Set<String> = [continueWatching, airingToday]

    /// Whether any of `kinds`, the widgets placed, is one of Kaeri's.
    public static func anyPlaced(_ kinds: some Sequence<String>) -> Bool {
        kinds.contains { all.contains($0) }
    }
}
