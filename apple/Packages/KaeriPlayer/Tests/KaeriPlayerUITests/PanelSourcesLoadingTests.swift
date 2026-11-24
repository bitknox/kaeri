// The Sources tab while it loads: every source's row from the engine's first SourcesProgress, each
// line as the last one says it, silent for 600 ms, then Asking and the seconds counted from when the
// row first said Asking, then the answer in the starting screen's row words.

@testable import KaeriPlayerUI
import KaeriKit
import KaeriStore
import KaeriStrings
import Foundation
import Testing

@MainActor
struct PanelSourcesLoadingTests {
    static func release(_ provider: String, _ id: String) -> SourceCandidate {
        SourceCandidate(
            id: id, providerId: provider, providerName: "Harbor Archive", title: "Stardust Wayfarer 07 [1080p]",
            quality: 1080, label: nil, size: nil, dualAudio: false, dubbed: false, autoPick: false)
    }

    static func row(_ id: String, _ state: ListingState) -> SourceListing {
        SourceListing(providerId: id, name: id.capitalized, state: state)
    }

    private func asked() async throws -> Rig {
        let rig = Rig()
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        try await rig.opened()
        rig.model.loadPanelReleases()
        return rig
    }

    @Test func eachProgressReplacesTheRowsWholeAndCountsFromTheFirstAsking() async throws {
        let rig = try await asked()
        let first = [
            Self.row("harbor", .asking), Self.row("shelf", .asking), Self.row("old", .off),
            Self.row("far", .missed(miss: .heldBack(why: .cellular))),
        ]
        rig.model.apply(.sourcesProgress(id: 900_001, episode: "7", rows: first))
        let started = rig.time.keeper.now()
        #expect(rig.model.panelReleases?.rows == first)
        #expect(rig.model.panelReleases?.askingSince["harbor"] == started)
        #expect(rig.model.panelReleases?.askingSince["old"] == nil, "a source switched off is never asked")

        rig.time.advance(by: .seconds(2))
        let second = [
            Self.row("harbor", .listed(releases: [Self.release("harbor", "r1")])), Self.row("shelf", .asking),
            Self.row("old", .off), Self.row("far", .missed(miss: .heldBack(why: .cellular))),
        ]
        rig.model.apply(.sourcesProgress(id: 900_001, episode: "7", rows: second))
        #expect(rig.model.panelReleases?.rows == second)
        #expect(rig.model.panelReleases?.askingSince["shelf"] == started, "the seconds go on from the first Asking")

        // The final answer follows the last progress and leaves its rows.
        rig.model.apply(.sources(id: 900_001, episode: "7", candidates: [Self.release("harbor", "r1")], missed: []))
        #expect(rig.model.panelReleases?.rows == second)
        #expect(rig.model.panelReleases?.candidates == [Self.release("harbor", "r1")])
    }

    @Test func progressForAnotherEpisodeIsLeftAlone() async throws {
        let rig = try await asked()
        rig.model.apply(.sourcesProgress(id: 900_001, episode: "8", rows: [Self.row("harbor", .asking)]))
        rig.model.apply(.sourcesProgress(id: 900_002, episode: "7", rows: [Self.row("harbor", .asking)]))
        #expect(rig.model.panelReleases?.rows == nil)
    }

    @Test func aRowIsSilentThenAsksThenCountsTheSeconds() {
        let asked = ContinuousClock.now
        let row = RowLoading(asked: asked, since: asked + .milliseconds(20))
        #expect(row.phase(at: asked + .milliseconds(300)) == .silent)
        #expect(row.phase(at: asked + .milliseconds(599)) == .silent)
        #expect(row.phase(at: asked + .milliseconds(650)) == .asking)
        #expect(row.phase(at: asked + .milliseconds(650)).text == L.playerStartRowAsking)
        #expect(row.phase(at: asked + .milliseconds(650)).spins)
        #expect(row.phase(at: asked + .milliseconds(3000)) == .asking, "3 s from when the row said Asking")
        #expect(row.phase(at: asked + .milliseconds(4300)) == .counting(4))
        #expect(row.phase(at: asked + .milliseconds(4300)).text == L.playerPanelAsking(seconds: 4))
        #expect(RowLoading.Phase.silent.text == nil)
        #expect(!RowLoading.Phase.silent.spins)
    }

    @Test func anAnswerShowsAtOnceEvenInsideTheSilentWait() {
        let asked = ContinuousClock.now
        let answered = RowLoading(asked: asked, said: "8 of 9 episodes")
        #expect(answered.phase(at: asked + .milliseconds(100)) == .said("8 of 9 episodes"))
        #expect(!answered.phase(at: asked).spins)
        #expect(!answered.asking)
    }

    @Test func eachStateSaysTheStartingScreensRowWords() {
        let releases = PanelReleases(id: 900_001, episode: "7")
        func said(_ state: ListingState, listed: String? = "9 of 9 episodes") -> String? {
            releases.loading(Self.row("harbor", state), listed: listed).said
        }
        #expect(said(.asking) == nil)
        #expect(said(.listed(releases: [Self.release("harbor", "r1")])) == "9 of 9 episodes")
        #expect(said(.listed(releases: [Self.release("harbor", "r1")]), listed: nil) == "")
        #expect(said(.off) == L.sourceNoteOff)
        #expect(said(.missed(miss: .noMatch)) == L.playerStartRowNoMatch)
        #expect(said(.missed(miss: .ambiguous)) == L.sourceNoteAmbiguous)
        #expect(said(.missed(miss: .noEpisode)) == L.playerStartRowNoEpisode(episode: "7"))
        #expect(said(.missed(miss: .failed)) == L.playerStartRowFailed)
        #expect(said(.missed(miss: .timedOut)) == L.playerStartRowTimedOut)
        #expect(said(.missed(miss: .heldBack(why: .cellular))) == L.playerStartRowHeldCellular)
        #expect(said(.missed(miss: .heldBack(why: .proxyCheck))) == L.playerStartRowHeldProxy)
    }

    @Test func aSourceThatCannotMatchTheTitleSaysSoInThePicker() {
        let missed = MissedListing(name: "Media folder", miss: .ambiguous)
        #expect(PartyWords.missed(missed, episode: "7") == L.playerStartMissAmbiguous(name: "Media folder"))
        #expect(!PartyWords.unreached(.ambiguous), "trying again does not choose the title")
    }
}
