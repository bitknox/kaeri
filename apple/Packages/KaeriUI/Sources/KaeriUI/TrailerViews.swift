// The trailer in a hover preview and on a title page: the video under the art, the sound button,
// the title page's measurements, and when a title page's visit starts and lets go.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The measurements of a title page that may play a trailer (Trailers boards, Mac, iPad, iPhone).
/// Room for the video is decided once per visit, so nothing moves when it starts or ends.
enum TrailerLayout {
    /// The video on the wide pages, 16:9 and above YouTube's 356 × 200 floor.
    static let video = CGSize(width: 448, height: 252)
    /// The ground under the band's art, as the page has it without a trailer: the cover and the
    /// words sit on it.
    static let ground: CGFloat = 96
    /// The sound button sits this far under the video's trailing corner.
    static let soundGap: CGFloat = 8
    /// How far the page scrolls before the video gives way to the art.
    static let scrollAway: CGFloat = 8
    /// A title kept this far from the video and the sound button.
    static let titleClearance: CGFloat = 16

    #if os(macOS)
    /// The art's band, from the window's top edge: the video's place at its trailing end.
    static let band: CGFloat = 328
    /// The video under the toolbar.
    static let videoTop: CGFloat = 60
    static let trailing: CGFloat = 28
    /// The cover's top: it overlaps the band by 88, as it does the banner without a trailer.
    static let coverTop: CGFloat = 240
    static let soundSize: CGFloat = 28
    #else
    static let band: CGFloat = 350
    static let videoTop: CGFloat = 82
    static let trailing: CGFloat = 24
    static let coverTop: CGFloat = 270
    static let soundSize: CGFloat = 44

    /// The iPhone's video, edge to edge across `width`, at 16:9.
    static func phoneVideo(width: CGFloat) -> CGSize {
        CGSize(width: width, height: (width * 9 / 16).rounded())
    }

    /// The iPhone's video sits 8 pt below the floating Back and More, whose bar ends at
    /// `barBottom` (the page's top safe area).
    static func phoneVideoTop(barBottom: CGFloat) -> CGFloat { barBottom + 8 }

    /// The iPhone's banner when a trailer may play: the video and 8 pt under it. 334 on a
    /// 390 pt wide iPhone whose bar ends at 98.
    static func phoneBand(width: CGFloat, barBottom: CGFloat) -> CGFloat {
        (phoneVideoTop(barBottom: barBottom) + phoneVideo(width: width).height + 8).rounded()
    }

    /// The iPhone's banner without a trailer, as today.
    static let phoneBanner: CGFloat = 250
    /// The head under the iPhone's band keeps this far clear of it.
    static let phoneHeadGap: CGFloat = 8
    #endif

    /// The video's rectangle on a wide page `width` wide, in the page's coordinates.
    static func videoFrame(width: CGFloat) -> CGRect {
        CGRect(x: width - trailing - video.width, y: videoTop, width: video.width, height: video.height)
    }

    /// The sound button's rectangle under the video's trailing corner.
    static func soundFrame(width: CGFloat) -> CGRect {
        let video = videoFrame(width: width)
        return CGRect(x: video.maxX - soundSize, y: video.maxY + soundGap, width: soundSize, height: soundSize)
    }
}

/// The video itself, which sits under the art: the art fades away over it. The web view takes no
/// pointer or touch, and nothing is drawn over it.
struct TrailerVideo: View {
    let visit: TrailerVisit
    let size: CGSize
    /// Whether it takes the width it is given, edge to edge.
    var fillsWidth = false

    var body: some View {
        ZStack {
            if let player = visit.surface as? TrailerPlayer, let view = player.webView {
                TrailerWebView(view: view)
            }
        }
        .frame(width: fillsWidth ? nil : size.width, height: size.height)
        .frame(maxWidth: fillsWidth ? .infinity : nil)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A plain icon button that turns the trailer's sound on or off. Never amber: the page's one
/// amber control is its main action. It fades in and out with the video.
struct TrailerSoundButton: View {
    let visit: TrailerVisit
    var size: CGFloat
    /// On a raised disc with a hairline ring, where it stands beside the title (iPhone).
    var raised = false

    var body: some View {
        Button {
            visit.toggleSound()
        } label: {
            Image(systemName: visit.muted ? "speaker.slash" : "speaker.wave.2")
                .font(.ui(size > 30 ? 17 : 13, weight: .medium))
                .foregroundStyle(.silk)
                .contentTransition(.symbolEffect(.replace))
                .frame(width: size, height: size)
                .background {
                    if raised {
                        Circle().fill(.raised).overlay(Circle().strokeBorder(.faintRule))
                    }
                }
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .animation(.quick, value: visit.muted)
        .tooltip(L.titleTrailerSound)
        .accessibilityLabel(L.titleTrailerSound)
        .accessibilityAddTraits(visit.muted ? [] : .isSelected)
        .opacity(visit.shown ? 1 : 0)
        .allowsHitTesting(visit.shown)
        .accessibilityHidden(!visit.shown)
        .animation(visit.shown ? .fadeIn : .fadeOut, value: visit.shown)
    }
}

/// The art over the video: it fades to the ground at the video's first frame (fade-in, 0.2 s) and
/// back when the trailer ends or the page scrolls away (fade-out, 0.3 s).
struct TrailerArtFade: ViewModifier {
    let visit: TrailerVisit?

    func body(content: Content) -> some View {
        let shown = visit?.shown ?? false
        content
            .opacity(shown ? 0 : 1)
            .animation(shown ? .fadeIn : .fadeOut, value: shown)
    }
}

/// When a title page's visit starts and when it lets go (trailers spec, behaviour 2, 7, 8 and 9).
struct TitleTrailerLifecycle: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.playerUp) private var playerUp
    #if os(macOS)
    @Environment(\.controlActiveState) private var activeState
    #endif

    let visit: TrailerVisit
    let entry: Entry
    /// Whether the page made room for a trailer this visit.
    let room: Bool
    /// Whether one may start now: the engine's answer, which can change while the page is up.
    let mayPlay: Bool
    /// From the page opening to the trailer loading: the page's own transition and half a second.
    var settle: Duration = .milliseconds(900)
    /// The title the visit was last for: another one in the same page starts a new visit.
    @State private var visited: AnilistId?

    private var sheetOpen: Bool {
        navigator.trailerRequest != nil || navigator.moreAbout != nil || navigator.sourcePicker != nil
            || navigator.showSettings != nil || navigator.messagesShown || navigator.matchPicker != nil
    }

    #if os(macOS)
    /// The window lost key status. The Fixtures app's stand-in keeps playing, so a shot can be
    /// taken of a window that is not in front.
    private var inactive: Bool { activeState == .inactive && !TrailerStandIn.installed }
    #else
    private var inactive: Bool { false }
    #endif

    private var blocked: Bool { playerUp || sheetOpen || scenePhase != .active || inactive }

    func body(content: Content) -> some View {
        content
            .task(id: Start(id: entry.id, play: room && mayPlay)) {
                if visited != entry.id {
                    if visited != nil { visit.reset() }
                    visited = entry.id
                }
                guard room, mayPlay, let trailer = entry.trailer else {
                    visit.release()
                    return
                }
                try? await Task.sleep(for: settle)
                guard !Task.isCancelled, !visit.live, !blocked else { return }
                let subtitles = store.settings.value(.subtitles) ?? ""
                visit.begin { signal in
                    TrailerPlayer(trailer: trailer, style: .banner, subtitles: subtitles, onSignal: signal)
                }
            }
            .onDisappear { visit.release() }
            .onChange(of: blocked) { _, now in if now { visit.release() } }
    }

    private struct Start: Hashable {
        let id: AnilistId
        let play: Bool
    }
}

extension View {
    /// A title page's trailer: starts once the page has settled, when it made room for one and
    /// one may play, and lets go when the page goes or anything comes over it.
    func titleTrailer(visit: TrailerVisit, entry: Entry, room: Bool, mayPlay: Bool) -> some View {
        modifier(TitleTrailerLifecycle(visit: visit, entry: entry, room: room, mayPlay: mayPlay))
    }

    func trailerArtFade(_ visit: TrailerVisit?) -> some View {
        modifier(TrailerArtFade(visit: visit))
    }
}
