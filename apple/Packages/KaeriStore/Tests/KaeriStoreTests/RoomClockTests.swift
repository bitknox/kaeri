// The room's clock writes a playhead as the player's clock and the engine's room lines do, so the
// room's header and its "paused at" line never disagree by a second.

@testable import KaeriStore
import Testing

struct RoomClockTests {
    @Test func wholeSecondsAreCutDownNotRounded() {
        #expect(RoomModel.clock(12.6) == "0:12", "the engine's line says 0:12 for 12.6 s")
        #expect(RoomModel.clock(59.99) == "0:59")
        #expect(RoomModel.clock(60) == "1:00")
    }

    @Test func anHourAndNothingAreWrittenAsThePlayerWritesThem() {
        #expect(RoomModel.clock(3729) == "1:02:09")
        #expect(RoomModel.clock(-3) == "0:00")
        #expect(RoomModel.clock(.nan) == "0:00")
    }

    @Test func theBannersEpisodeLineEndsWithTheLengthWhenKnown() {
        #expect(RoomCopy.bannerEpisode("9", "The Long Leg", minutes: 24) == "Episode 9 · The Long Leg · 24 min")
        #expect(RoomCopy.bannerEpisode("9", nil, minutes: 24) == "Episode 9 · 24 min")
        #expect(RoomCopy.bannerEpisode("9", "The Long Leg", minutes: nil) == "Episode 9 · The Long Leg")
        #expect(RoomCopy.bannerEpisode("9", nil, minutes: 0) == "Episode 9")
    }
}
