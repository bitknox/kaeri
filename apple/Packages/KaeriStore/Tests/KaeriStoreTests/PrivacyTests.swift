// Private mode as the store keeps it: the switch, the line under a title's main button, the
// notice a menu item puts up, and the system surfaces held while it is on. No engine: events
// are applied by hand, and the Live Activities go to fake hosts.

@testable import KaeriStore
import KaeriKit
import KaeriStrings
import KaeriWidgetData
import Foundation
import Testing

@MainActor
struct PrivacyTests {
    let id: AnilistId = 900_001

    // MARK: The mode and the line

    @Test func theModeIsWhatTheEngineLastSaid() {
        let store = AppStore()
        #expect(!store.privacy.on, "a launch starts with it off")
        store.apply(.private(true))
        #expect(store.privacy.on)
        #expect(store.privacy.holds(show: id))
        store.apply(.private(false))
        #expect(!store.privacy.on)
        #expect(!store.privacy.holds(show: id))
    }

    @Test func theLineFollowsTheModeAsItIsNow() {
        let store = AppStore()
        #expect(store.privacy.line(show: id) == nil)
        store.apply(.private(true))
        #expect(store.privacy.line(show: id) == PrivateLine(mode: .session, kept: false))
        // Sent while on, then turned off: the line goes with the mode.
        store.apply(.progress(id: id, progress: nil, resume: nil, episodes: [], private: .session, kept: false))
        store.apply(.private(false))
        #expect(store.privacy.line(show: id) == nil)
    }

    @Test func aPlaceHeldInMemorySaysSo() {
        let store = AppStore()
        store.apply(.private(true))
        store.apply(.progress(id: id, progress: nil, resume: nil, episodes: [], private: .session, kept: true))
        #expect(store.privacy.line(show: id) == PrivateLine(mode: .session, kept: true))
        store.apply(.progress(id: id, progress: nil, resume: nil, episodes: [], private: .session, kept: false))
        #expect(store.privacy.line(show: id)?.kept == false)
    }

    @Test func alwaysPrivateComesFirstWhetherTheModeIsOnOrNot() {
        let store = AppStore()
        store.apply(.alwaysPrivate(title: .show(id: id), on: true))
        #expect(store.privacy.line(show: id) == PrivateLine(mode: .always, kept: false))
        #expect(store.privacy.holds(show: id))
        #expect(!store.privacy.holds(show: 900_002))
        store.apply(.private(true))
        #expect(store.privacy.line(show: id)?.mode == .always)
        #expect(store.privacy.line(show: 900_002)?.mode == .session)
        store.apply(.alwaysPrivate(title: .show(id: id), on: false))
        #expect(store.privacy.line(show: id)?.mode == .session)
    }

    @Test func theShowsOwnViewsSayItToo() {
        let store = AppStore()
        store.apply(.progress(id: id, progress: nil, resume: nil, episodes: [], private: .always, kept: false))
        #expect(store.privacy.isAlways(show: id))
        store.apply(.playbackStaged(id: 900_003, title: "Lanterns of the Old Quarter", episode: "2", episodeTitle: nil, speed: 1, alwaysPrivate: true))
        #expect(store.privacy.isAlways(show: 900_003))
        store.apply(.progress(id: id, progress: nil, resume: nil, episodes: [], private: nil, kept: false))
        #expect(!store.privacy.isAlways(show: id))
    }

    @Test func aSeriesLineIsFromItsView() {
        let store = AppStore()
        store.apply(.alwaysPrivate(title: .series(work: "a:30001"), on: true))
        #expect(store.privacy.isAlways(series: "a:30001"))
        #expect(store.privacy.holds(series: "a:30001"))
        #expect(!store.privacy.holds(series: "a:30002"))
        store.apply(.alwaysPrivate(title: .series(work: "a:30001"), on: false))
        #expect(!store.privacy.isAlways(series: "a:30001"))
    }

    @Test func shownIsTheEnginesRule() {
        #expect(PrivacyModel.shown(.always, on: false) == .always)
        #expect(PrivacyModel.shown(.always, on: true) == .always)
        #expect(PrivacyModel.shown(.session, on: false) == nil, "turned off since")
        #expect(PrivacyModel.shown(nil, on: true) == .session, "turned on since")
        #expect(PrivacyModel.shown(nil, on: false) == nil)
    }

    // MARK: Turning it on and off

    @Test func theSwitchTurnsItOnAtOnceWithNoNotice() {
        let store = AppStore()
        store.setPrivate(true)
        #expect(store.privacy.on)
        #expect(store.notices.current == nil)
        store.setPrivate(false)
        #expect(!store.privacy.on)
    }

    @Test func fromTheMenuANoticeSaysWhatItMeansWithTurnOff() throws {
        let store = AppStore()
        store.setPrivate(true, notice: true)
        let notice = try #require(store.notices.current)
        #expect(notice.notice.text == L.privateModeOnNotice)
        #expect(notice.action?.title == L.actionTurnOff)
        #expect(notice.glyph == "eye.slash")
        #expect(!notice.spoken, "the shorter announcement is read instead")
        notice.action?.run()
        #expect(!store.privacy.on)
        #expect(store.notices.current == nil, "turned off, the notice goes")
    }

    @Test func turnedOffElsewhereTheNoticeGoesToo() {
        let store = AppStore()
        store.setPrivate(true, notice: true)
        #expect(store.notices.current != nil)
        store.apply(.private(false))
        #expect(store.notices.current == nil)
    }

    @Test func alwaysPrivateIsSetAtOnce() {
        let store = AppStore()
        store.setAlwaysPrivate(show: id, true)
        #expect(store.privacy.isAlways(show: id))
        store.setAlwaysPrivate(series: "a:30001", true)
        #expect(store.privacy.isAlways(series: "a:30001"))
        store.setAlwaysPrivate(show: id, false)
        #expect(!store.privacy.isAlways(show: id))
    }

    @Test func theEventsGoOnToThePlayer() {
        let store = AppStore()
        var seen: [Event] = []
        store.onPlayback = { seen.append($0) }
        store.apply(.private(true))
        store.apply(.alwaysPrivate(title: .show(id: id), on: true))
        #expect(seen == [.private(true), .alwaysPrivate(title: .show(id: id), on: true)])
    }

    // MARK: What the system is not told

    @Test func noLiveActivityNamesAPrivatePlay() {
        let host = FakeLiveActivityHost<PlaybackActivityState>()
        let store = AppStore(playbackActivity: PlaybackActivityFeed(host: host, files: nil, now: { Date() }))
        let playing = PlaybackActivityFeed.Playing(
            id: id, title: "Hoshikuzu no Tabibito", episode: "7", position: 10, duration: 1_400, rate: 1, paused: false)
        store.apply(.private(true))
        store.playbackChanged(playing)
        #expect(host.calls.isEmpty)
        store.apply(.private(false))
        store.apply(.alwaysPrivate(title: .show(id: id), on: true))
        store.playbackChanged(playing)
        #expect(host.calls.isEmpty)
        store.apply(.alwaysPrivate(title: .show(id: id), on: false))
        store.playbackChanged(playing)
        #expect(!host.calls.isEmpty)
    }

    @Test func aPrivateDownloadShowsNoLiveActivity() {
        let host = FakeActivityHost()
        let feed = DownloadsActivityFeed(host: host, files: nil, now: { Date() })
        let model = DownloadsModel()
        var row = download(1, "8", .active, done: 100_000_000)
        row.private = true
        model.apply([row], at: Date())
        feed.downloadsChanged(model, titles: TitleCache(), artwork: nil)
        #expect(host.calls.isEmpty)
    }
}
