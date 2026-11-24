// The Episodes panel's line under a row that is not playing: its length, what is left of it, or
// Watched, from the same table and in the same minutes as the title page's rows.

@testable import KaeriPlayerUI
import KaeriKit
import KaeriStrings
import Testing

struct PanelEpisodeLineTests {
    private func row(_ number: String, seconds: UInt64?, watched: Double = 0, completed: Bool = false) -> EpisodeRow {
        EpisodeRow(
            number: number, title: "The Lantern Road, part \(number)", thumbnail: nil, description: nil,
            durationSecs: seconds, watched: watched, completed: completed, kind: nil, skippable: false, airsAt: nil)
    }

    @Test func aRowNobodyStartedSaysItsLengthInMinutes() {
        // A 24-minute episode, as the title page says it, and one of 23:40.
        #expect(PanelEpisodeRow.standing(row("20", seconds: 1_440), metadataOnly: false) == L.episodeRowLength(minutes: 24))
        #expect(PanelEpisodeRow.standing(row("21", seconds: 1_420), metadataOnly: false) == L.episodeRowLength(minutes: 24))
        #expect(PanelEpisodeRow.standing(row("22", seconds: 1_440), metadataOnly: false) != L.episodeRowLength(minutes: 1))
    }

    @Test func aStartedRowSaysWhatIsLeftAndAFinishedOneSaysWatched() {
        #expect(PanelEpisodeRow.standing(row("19", seconds: 1_440, watched: 0.5), metadataOnly: false)
            == L.timeMinutesLeft(minutes: 12))
        #expect(PanelEpisodeRow.standing(row("18", seconds: 1_440, watched: 1, completed: true), metadataOnly: false)
            == L.episodeWatched)
        // A table from AniList alone shows no progress: the length stands.
        #expect(PanelEpisodeRow.standing(row("17", seconds: 1_440, watched: 0.5), metadataOnly: true)
            == L.episodeRowLength(minutes: 24))
    }

    @Test func aRowWithNoLengthSaysNothing() {
        #expect(PanelEpisodeRow.standing(row("23", seconds: nil), metadataOnly: false) == nil)
        #expect(PanelEpisodeRow.standing(row("23", seconds: 0), metadataOnly: false) == nil)
    }

    @Test func theLengthInSecondsIsTheTablesNumber() {
        #expect(row("20", seconds: 1_440).seconds == 1_440)
        #expect(row("20", seconds: nil).seconds == nil)
    }
}
