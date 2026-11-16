// What Now Playing says of a TV the Mac drives or casts to.

#if os(macOS)
import KaeriKit
import MediaPlayer
import Testing

@testable import KaeriPlayerUI

@MainActor
struct TVNowPlayingTests {
    func card(paused: Bool = false, loading: Bool = false) -> FfiNowCard {
        FfiNowCard(
            id: 900_001, title: "Stardust Wayfarer", episode: "2", episodeLine: "Episode 2 · The Night Platform",
            loading: loading, position: 30, duration: 1440, paused: paused, speed: 1.25, volume: 40,
            audio: [], subtitles: [], subtitlesOff: false, skip: nil)
    }

    @Test func theTVsEpisodeIsShownAtItsPlayhead() {
        let info = TVNowPlaying.info(card(), tvName: "Living room", playhead: 42, live: true)
        #expect(info[MPMediaItemPropertyTitle] as? String == "Episode 2 · The Night Platform")
        #expect(info[MPMediaItemPropertyArtist] as? String == "Stardust Wayfarer")
        #expect(info[MPMediaItemPropertyAlbumTitle] as? String == "Playing on Living room")
        #expect(info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double == 42)
        #expect(info[MPMediaItemPropertyPlaybackDuration] as? Double == 1440)
        #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 1.25)
    }

    @Test func whilePrivateItNamesNothing() {
        let info = TVNowPlaying.info(card(), tvName: "Living room", playhead: 42, live: true, private: true)
        #expect(info[MPMediaItemPropertyTitle] as? String == "Playing in Kaeri")
        #expect(info[MPMediaItemPropertyArtist] == nil)
        #expect(info[MPMediaItemPropertyAlbumTitle] == nil)
        #expect(info[MPMediaItemPropertyArtwork] == nil)
        // The controls keep working: the clock still runs.
        #expect(info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double == 42)
        #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 1.25)
    }

    @Test func theClockStopsWhilePausedStartingOrLost() {
        for (now, live) in [(card(paused: true), true), (card(loading: true), true), (card(), false)] {
            #expect(!TVNowPlaying.playing(now, live: live))
            let info = TVNowPlaying.info(now, tvName: "Living room", playhead: 42, live: live)
            #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 0)
        }
        #expect(TVNowPlaying.playing(card(), live: true))
    }

    func item(paused: Bool = false) -> FfiCastItem {
        FfiCastItem(
            id: 900_001, title: "Stardust Wayfarer", episode: "2", episodeTitle: "The Night Platform", image: nil,
            position: 30, duration: 1440, paused: paused, waiting: nil, loading: false)
    }

    @Test func aCastsEpisodeSaysWhichTvPlaysIt() {
        let info = TVNowPlaying.info(item(), line: "Episode 2 · The Night Platform", tvName: "Living room", playhead: 42)
        #expect(info[MPMediaItemPropertyTitle] as? String == "Episode 2 · The Night Platform")
        #expect(info[MPMediaItemPropertyArtist] as? String == "Stardust Wayfarer")
        #expect(info[MPMediaItemPropertyAlbumTitle] as? String == "Casting to Living room")
        #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 1)
        #expect(TVNowPlaying.info(item(paused: true), line: "", tvName: "Living room", playhead: 42)[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 0)
    }

    @Test func aPrivateCastNamesNothing() {
        let info = TVNowPlaying.info(item(), line: "Episode 2 · The Night Platform", tvName: "Living room", playhead: 42, private: true)
        #expect(info[MPMediaItemPropertyTitle] as? String == "Playing in Kaeri")
        #expect(info[MPMediaItemPropertyArtist] == nil)
        #expect(info[MPMediaItemPropertyAlbumTitle] == nil)
        #expect(info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double == 42)
    }

    @Test func theArtIsMadeForTheSystemToAskOffTheMainThread() async {
        let context = CGContext(
            data: nil, width: 4, height: 2, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        let artwork = TVNowPlaying.artwork(SendableImage(image: context.makeImage()!))
        #expect(artwork.bounds.size == CGSize(width: 4, height: 2))
        let image = await Task.detached { artwork.image(at: CGSize(width: 4, height: 2)) }.value
        #expect(image != nil)
    }
}
#endif
