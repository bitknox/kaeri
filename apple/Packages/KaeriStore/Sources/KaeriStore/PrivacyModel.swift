// Private watching and reading: whether private mode is on, which titles are always private on
// this device, and what each title page's line says.

import KaeriKit
import Foundation
import Observation

/// What the engine said about private mode, kept for the screens and for the system surfaces
/// that must not name what is open: Now Playing, Live Activities, Handoff, Spotlight and widgets.
@Observable @MainActor
public final class PrivacyModel {
    /// Whether private mode is on: never saved, so every launch starts with it off.
    public private(set) var on = false
    /// The shows always private on this device that the engine has named this session.
    public private(set) var alwaysShows: Set<AnilistId> = []
    /// The series always private on this device that the engine has named this session.
    public private(set) var alwaysSeries: Set<WorkId> = []
    /// The shows whose page continues from a place held in memory until the app quits.
    public private(set) var keptShows: Set<AnilistId> = []
    /// Each show's privacy as its last progress said it.
    private var shown: [AnilistId: PrivateMode] = [:]

    public init() {}

    /// Take what an event says about privacy.
    func apply(_ event: Event) {
        switch event {
        case .private(let on):
            self.on = on
        case .alwaysPrivate(.show(let id), let on):
            set(show: id, always: on)
        case .alwaysPrivate(.series(let work), let on):
            if on { alwaysSeries.insert(work) } else { alwaysSeries.remove(work) }
        case .progress(let id, _, _, _, let mode, let kept):
            shown[id] = mode
            // The view's own flag comes first, so anything else says the show is not always private.
            if mode == .always { alwaysShows.insert(id) } else { alwaysShows.remove(id) }
            if kept { keptShows.insert(id) } else { keptShows.remove(id) }
        case .playbackStaged(let id, _, _, _, _, _, let always):
            if always { alwaysShows.insert(id) }
        case .showSettings(let view):
            set(show: view.id, always: view.alwaysPrivate)
        case .reading(.series(let view)):
            set(series: view.work, always: view.private == .always)
        default:
            break
        }
    }

    /// Turn private mode on or off here at once, before the engine answers.
    func set(on: Bool) { self.on = on }

    func set(show id: AnilistId, always: Bool) {
        if always {
            alwaysShows.insert(id)
            shown[id] = .always
        } else {
            alwaysShows.remove(id)
            if shown[id] == .always { shown[id] = nil }
        }
    }

    func set(series work: WorkId, always: Bool) {
        if always { alwaysSeries.insert(work) } else { alwaysSeries.remove(work) }
    }

    /// A place held in memory for `id`, as the engine says after a play while private: for the
    /// Fixtures app's shots.
    func stageKept(_ id: AnilistId) {
        keptShows.insert(id)
    }

    /// Whether `id` is always private on this device.
    public func isAlways(show id: AnilistId) -> Bool { alwaysShows.contains(id) }

    /// Whether `work` is always private on this device.
    public func isAlways(series work: WorkId) -> Bool { alwaysSeries.contains(work) }

    /// Whether nothing of `id` may show outside the app: private mode is on, or the show is
    /// always private.
    public func holds(show id: AnilistId) -> Bool { on || isAlways(show: id) }

    /// Whether nothing of `work` may show outside the app.
    public func holds(series work: WorkId) -> Bool { on || isAlways(series: work) }

    /// The line under a show page's main button, or `nil` for none.
    public func line(show id: AnilistId) -> PrivateLine? {
        let view: PrivateMode? = isAlways(show: id) ? .always : shown[id]
        return PrivateLine(mode: Self.shown(view, on: on), kept: keptShows.contains(id))
    }

    /// The line under a series page's main button, from what its view said.
    public func line(series view: SeriesView) -> PrivateLine? {
        let mode: PrivateMode? = isAlways(series: view.work) ? .always : view.private
        return PrivateLine(mode: Self.shown(mode, on: on), kept: view.placeKept)
    }

    /// What a title page shows from what its view said and whether the mode is on now: the
    /// title's own flag first, then the mode (the engine's `PrivateMode::shown`).
    nonisolated public static func shown(_ view: PrivateMode?, on: Bool) -> PrivateMode? {
        if view == .always { return .always }
        return on ? .session : nil
    }
}

/// The line under a title or series page's main button while it is private.
public struct PrivateLine: Equatable, Sendable {
    /// Why: the mode or the title's own flag.
    public let mode: PrivateMode
    /// The main button continues from a place held in memory until the app quits.
    public let kept: Bool

    init?(mode: PrivateMode?, kept: Bool) {
        guard let mode else { return nil }
        self.mode = mode
        self.kept = kept
    }

    public init(mode: PrivateMode, kept: Bool) {
        self.mode = mode
        self.kept = kept
    }
}
