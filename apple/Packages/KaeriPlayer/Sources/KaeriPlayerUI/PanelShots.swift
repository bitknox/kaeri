// The Episodes and Sources panel's shots and recording on the Fixtures app's Playing scene, with
// made-up sources and releases (player panel board). Testing only.

#if FIXTURES
import Foundation
import KaeriKit
import KaeriStore

/// The board's sources: the one playing, one with five releases of the episode, and four with
/// nothing to pick, for each reason a source rests.
enum PanelFixture {
    static let playing = "Example source"
    static let index = "Example index"

    static func options() -> [SourceOption] {
        [
            SourceOption(providerId: "ex", name: playing, covered: 8, of: 9),
            SourceOption(providerId: "home", name: "Home server", covered: 9, of: 9),
            SourceOption(providerId: "idx", name: index, covered: 9, of: 9),
            SourceOption(providerId: "fold", name: "Media folder", covered: 0, of: 9, note: .ambiguous),
            SourceOption(providerId: "sec", name: "Second example", covered: 0, of: 9, note: .resting),
            SourceOption(providerId: "arc", name: "Archive example", covered: 0, of: 9, note: .failed(reason: "HTTP 503 from api.archive.example")),
            SourceOption(providerId: "off", name: "Old example", covered: 0, of: 9, note: .off),
        ]
    }

    static func releases(_ title: String, _ episode: String) -> [SourceCandidate] {
        let number = episode.count < 2 ? "0" + episode : episode
        func release(_ id: String, _ name: String, quality: UInt32, size: String, label: String? = nil, dual: Bool = false, dub: Bool = false, auto: Bool = false) -> SourceCandidate {
            SourceCandidate(
                id: id, providerId: "idx", providerName: index, title: name, quality: quality, label: label, size: size,
                dualAudio: dual, dubbed: dub, autoPick: auto)
        }
        return [
            SourceCandidate(id: "h1", providerId: "home", providerName: "Home server", title: "\(title) \(number).mkv", quality: 1080, label: nil, size: "1.2 GiB", dualAudio: false, dubbed: false, autoPick: false),
            release("r1", "\(title) \(number) 1080p", quality: 1080, size: "1.4 GiB", label: "lantern-archive", auto: true), // i18n-skip
            release("r2", "\(title) \(number) 1080p multi", quality: 1080, size: "2.1 GiB", dual: true),
            release("r3", "\(title) \(number) dub 1080p", quality: 1080, size: "1.5 GiB", dub: true),
            release("r4", "\(title) \(number) 720p", quality: 720, size: "620 MiB", label: "night-index"), // i18n-skip
            release("r5", "\(title) 01 to 13 1080p", quality: 1080, size: "18.2 GiB", label: "batch"), // i18n-skip
        ]
    }

    /// A made-up synopsis long enough to run past the row's two lines.
    static let synopsis = "A letter turns up in the lamplighter's coat, addressed to a keeper who left the star road years ago. " // i18n-skip
        + "The road home goes the long way round, past every lamp still lit, and one that has been dark since the first night." // i18n-skip

    static func to(_ release: SourceCandidate) -> SwitchTo {
        SwitchTo(providerId: release.providerId, name: release.providerName, release: release.id, stream: nil)
    }
}

extension PlayerModel {
    /// The panel open in `shot`'s state over the Playing scene's episode at 31 s.
    func stagePanel(_ shot: PlayerShot, id: AnilistId, episode: String, store: AppStore) async -> Bool {
        // The recording starts earlier, so its 21 s end before Up next comes at 43 s.
        let at: Double = shot == .panelMotion ? 12 : 31
        seek(to: at)
        var there = await waited(10) { self.position >= at + 0.5 && self.menus != nil }
        // Stills are paused; the recording plays, so the held picture shows against the moving one.
        if shot != .panelMotion && shot != .panelLive { togglePause() }
        // Spoiler protection off for the show, as on the board: the revealed row shows its still
        // and synopsis rather than the protected line.
        store.setShowSetting(id, .spoilerProtection, to: .bool(false))
        store.loadEpisodes(id)
        _ = await waited(10) { store.titles.episodes[id]?.isEmpty == false }
        let next = String((Int(episode) ?? 1) + 1)
        // The fixture lists a synopsis for the first episode only; the revealed one gets a long
        // one, so the reveal shows its whole text. Laid on again below, over a later listing.
        func describeNext() {
            guard var rows = store.titles.episodes[id], let at = rows.firstIndex(where: { $0.number == next }),
                  rows[at].description == nil else { return }
            rows[at].description = PanelFixture.synopsis
            store.apply(.episodes(id: id, rows: rows, failed: nil))
        }
        describeNext()
        if shot == .panelLive {
            panelShot = PanelShot(tab: .sources, serial: 1)
            requestMenu(.episodes)
            loadPanelReleases(again: true)
            there = await waited(20) { self.panelReleases?.candidates != nil }
            let other = panelReleases?.candidates?.first { $0.providerName != self.playingSource }
                ?? panelReleases?.candidates?.first
            guard let other else { return false }
            panelShot = PanelShot(tab: .sources, expanded: other.providerId, serial: 2)
            try? await Task.sleep(for: .seconds(1))
            switchTo(other)
            return await waited(30) { self.panelSwitch == nil || self.panelSwitch?.failed != nil }
        }
        store.apply(.sourceOptions(id: id, options: PanelFixture.options()))
        let releases = PanelFixture.releases(staged?.title ?? "", episode)
        panelReleases = PanelReleases(id: id, episode: episode, candidates: releases)
        playingSource = PanelFixture.playing
        let multi = releases[2], small = releases[4]
        // The panel asks the engine for the title's sources as it opens: the made-up ones go in
        // again over its answer.
        Task { @MainActor in
            for _ in 0 ..< 24 {
                try? await Task.sleep(for: .milliseconds(500))
                store.apply(.sourceOptions(id: id, options: PanelFixture.options()))
                describeNext()
            }
        }
        switch shot {
        case .panelEpisodes:
            panelShot = PanelShot(tab: .episodes, revealed: next)
            requestMenu(.episodes)
        case .panelSources:
            panelShot = PanelShot(tab: .sources, expanded: "idx")
            requestMenu(.episodes)
        case .panelFinding:
            panelShot = PanelShot(tab: .sources, expanded: "idx")
            requestMenu(.episodes)
            panelSwitch = PanelSwitch(
                target: SwitchTarget(PanelFixture.to(multi)), phase: .finding, since: .now - .milliseconds(4200), candidate: multi)
        case .panelLoading, .panelLanded, .panelFailed:
            let pick = shot == .panelFailed ? small : multi
            panelShot = PanelShot(tab: .sources, expanded: "idx")
            requestMenu(.episodes)
            try? await Task.sleep(for: .seconds(0.6))
            panelSwitch = PanelSwitch(target: SwitchTarget(PanelFixture.to(pick)), phase: .asked, since: .now, candidate: pick)
            engineSays(.finding(to: PanelFixture.to(pick)))
            try? await Task.sleep(for: .seconds(0.8))
            engineSays(.loading(to: PanelFixture.to(pick), at: position))
            there = await waited(5) { self.heldPicture != nil } || there
            if shot == .panelLanded {
                try? await Task.sleep(for: .seconds(0.4))
                engineSays(.landed(to: PanelFixture.to(pick), at: position))
            } else if shot == .panelFailed {
                try? await Task.sleep(for: .seconds(0.4))
                engineSays(.failed(
                    to: PanelFixture.to(pick), at: position,
                    text: .playerPanelSwitchFailed(source: PanelFixture.index, current: PanelFixture.playing), reason: nil))
            }
        case .panelMotion:
            await panelMotion(next: next, multi: multi, home: releases[0])
        default:
            break
        }
        try? await Task.sleep(for: .seconds(0.8))
        return there
    }

    /// The recording: the panel opens on Episodes, reveals the next episode, goes to Sources,
    /// opens a source's releases, switches to one (finding, the spinner past 600 ms, "Starting"
    /// from 3 s, loading over the held picture, landed), then to another source that fails.
    private func panelMotion(next: String, multi: SourceCandidate, home: SourceCandidate) async {
        func pause(_ seconds: Double) async { try? await Task.sleep(for: .seconds(seconds)) }
        await pause(1.5)
        panelShot = PanelShot(tab: .episodes, serial: 1)
        requestMenu(.episodes)
        await pause(1.6)
        panelShot = PanelShot(tab: .episodes, revealed: next, serial: 2)
        await pause(2)
        panelShot = PanelShot(tab: .sources, revealed: next, serial: 3)
        await pause(1.4)
        panelShot = PanelShot(tab: .sources, revealed: next, expanded: "idx", serial: 4)
        await pause(1.4)
        for (pick, lands) in [(multi, true), (home, false)] {
            let to = PanelFixture.to(pick)
            if !lands {
                // The viewer scrolls down to the next source to try.
                panelShot = PanelShot(tab: .sources, revealed: next, expanded: pick.providerId, serial: 5)
                await pause(1.4)
            }
            panelSwitch = PanelSwitch(target: SwitchTarget(to), phase: .asked, since: .now, candidate: pick)
            await pause(0.3)
            engineSays(.finding(to: to))
            await pause(lands ? 3.7 : 1.2)
            engineSays(.loading(to: to, at: position))
            await pause(1.3)
            if lands {
                engineSays(.landed(to: to, at: position))
            } else {
                engineSays(.failed(
                    to: to, at: position,
                    text: .playerPanelSwitchFailed(source: pick.providerName, current: multi.providerName), reason: nil))
            }
            await pause(2.4)
        }
    }

    /// The engine's PlayerSwitch for the playback on screen, as it would say it.
    private func engineSays(_ state: SourceSwitch) {
        guard let playback else { return }
        apply(.playerSwitch(playback: playback, state: state))
    }
}
#endif
