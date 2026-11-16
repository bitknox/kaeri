// A cover filling a wide frame, for a title with no banner and no seen still: cropped around the
// part of it that draws the eye, or, where it is too small to crop, blurred behind itself (the
// hover card board's No wide art rule). Calendar tiles, the hover card's band, the Mac's title
// page header, and the iPhone's Continue cards, spotlight and party screens draw it. Where a
// frame on iPhone has no cover to fill it, a plate in the cover's colour carries the title.

import KaeriDesign
import KaeriKit
import KaeriStore
import CoreGraphics
import Observation
import SwiftUI
import Vision

/// How a cover fills a wide frame.
enum CoverFillMode: Equatable {
    /// Cropped to fill, around its focus.
    case crop
    /// A blurred, darkened enlargement, with the sharp cover on top where the frame asks for it.
    case blur

    /// The most a crop may enlarge the file, at the screen's scale. Every cover in the fixture
    /// catalogue is 460 px wide, AniList's largest size, which the 384 pt hover band at 2x asks
    /// 1.67 times of: under this, so the band crops. A title page header, three times and more,
    /// blurs.
    static let largest: CGFloat = 1.75

    /// The most a crop may enlarge the file in a card on iPhone and iPad, a frame `cardWidth`
    /// points wide or less. A 460 px cover in the iPhone's 260 pt card at 3x asks 1.7 times: over
    /// this, so it blurs, sharp in the middle. At 2x it crops.
    static let card: CGFloat = 1.6
    static let cardWidth: CGFloat = 300

    /// How far a crop may enlarge the file in a frame `points` wide. The Mac keeps `largest`
    /// everywhere: its cards, at 2x, never come near either limit with a full-size cover.
    static func limit(points: CGFloat?, touch: Bool = CoverFillMode.touch) -> CGFloat {
        guard touch, let points, points <= cardWidth else { return largest }
        return card
    }

    /// Whether this is an iPhone or iPad build.
    static var touch: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    /// The mode for a frame `pixels` wide (`points` at the screen's scale, when the frame's size
    /// in points is known), from a file `source` pixels wide; a crop while the file's size is not
    /// known yet, as nearly every cover crops.
    static func of(
        pixels: CGFloat, source: Int?, points: CGFloat? = nil, touch: Bool = CoverFillMode.touch
    ) -> CoverFillMode {
        guard let source, source > 0 else { return .crop }
        return pixels / CGFloat(source) > limit(points: points, touch: touch) ? .blur : .crop
    }

    /// Where the top of a cover `width` wide sits in a frame `height` high so its focus, a
    /// fraction of its height from the top, is at the frame's middle, without showing past either
    /// end: the upper third gives the hover band's 84 pt.
    static func cropTop(width: CGFloat, height: CGFloat, focus: CGFloat) -> CGFloat {
        let tall = width * 3 / 2
        return min(0, max(height - tall, height / 2 - tall * focus))
    }
}

/// Where each cover's eye-catching part is, from Vision's attention saliency, worked out once per
/// cover in a run and kept.
@MainActor @Observable
final class CoverFocus {
    static let shared = CoverFocus()

    /// The upper third: where a cover's focus is until Vision has said, and when it finds none.
    nonisolated static let fallback: CGFloat = 1 / 3

    @ObservationIgnored private var found: [String: Found] = [:]
    @ObservationIgnored private var working: Set<String> = []

    /// One cover's answer, observed apart so an answer redraws only the views showing it.
    @Observable
    final class Found {
        var focus: CGFloat?
    }

    private func entry(_ url: String) -> Found {
        if let found = found[url] { return found }
        let new = Found()
        found[url] = new
        return new
    }

    /// The focus of `url`'s cover, a fraction of its height from the top, once known.
    func focus(_ url: String) -> CGFloat? {
        entry(url).focus
    }

    /// Where Vision works out each focus: one cover at a time, on a queue of its own.
    ///
    /// Never on Swift's concurrency pool. Vision's `perform` blocks its thread until work it hands
    /// on has run, and that work needs a thread of the same pool. A screen of covers landing at
    /// once, one per core, would take every thread of the pool, each waiting on work no thread is
    /// left to run, and stop everything else the pool runs for good: every cover's decode, so no
    /// art lands again, and the engine's events.
    private nonisolated static let vision = DispatchQueue(label: "app.kaeri.cover-focus", qos: .utility) // i18n-skip

    /// Work out `url`'s focus from its decoded art, once.
    func prepare(_ url: String, image: CGImage) {
        guard entry(url).focus == nil, working.insert(url).inserted else { return }
        let held = HeldImage(image: image)
        Self.vision.async {
            let focus = Self.saliency(of: held.image) ?? Self.fallback
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    self.entry(url).focus = focus
                    self.working.remove(url)
                }
            }
        }
    }

    /// The middle of the most salient region, from the top, or nil when Vision finds none.
    nonisolated static func saliency(of image: CGImage) -> CGFloat? {
        let request = VNGenerateAttentionBasedSaliencyImageRequest()
        let handler = VNImageRequestHandler(cgImage: image)
        do { try handler.perform([request]) } catch { return nil }
        guard let observation = request.results?.first,
              let best = observation.salientObjects?.max(by: { $0.confidence < $1.confidence })
        else { return nil }
        // Vision's boxes are normalised with the origin at the bottom left.
        return 1 - best.boundingBox.midY
    }

    /// A decoded image carried to Vision's thread: a CGImage is immutable once made.
    private struct HeldImage: @unchecked Sendable {
        let image: CGImage
    }
}

/// A cover filling the frame it is given, by `CoverFillMode`.
struct CoverFill: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let entry: Entry
    /// Whether the sharp cover sits on the blurred one: not where the screen shows the cover
    /// beside the frame itself, as a title page header does.
    var sharpCover = true

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            if let url = entry.coverUrl {
                let mode = CoverFillMode.of(
                    pixels: size.width * scale, source: store.artwork.sourceWidth(url), points: size.width
                )
                switch mode {
                case .crop:
                    CoverCrop(entry: entry, url: url, size: size)
                case .blur:
                    BlurredCover(entry: entry, size: size, sharpCover: sharpCover)
                }
            } else {
                CoverPlate(color: entry.plateColor)
            }
        }
        .clipped()
    }
}

/// The cover cropped to fill `size` around its focus. Its art shows once the focus is known, a
/// moment after it lands, so the crop never moves.
struct CoverCrop: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let url: String
    let size: CGSize

    var body: some View {
        let focus = CoverFocus.shared.focus(url)
        let pixels = Int(size.width * 1.5 * scale)
        ArtworkView(url: focus == nil ? nil : url, color: entry.plateColor, maxPixelSize: pixels)
            .frame(width: size.width, height: size.width * 3 / 2)
            .offset(y: CoverFillMode.cropTop(width: size.width, height: size.height, focus: focus ?? CoverFocus.fallback))
            .frame(width: size.width, height: size.height, alignment: .top)
            .task(id: url) { await store.artwork.show(url, maxPixelSize: pixels) }
            .onChange(of: store.artwork.image(url) != nil, initial: true) { _, here in
                if here, let image = store.artwork.image(url) { CoverFocus.shared.prepare(url, image: image) }
            }
    }
}

/// A blurred, darkened enlargement of the cover filling `size`, and the sharp cover centred on it
/// at 86% of the height.
struct BlurredCover: View {
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let size: CGSize
    var sharpCover = true

    var body: some View {
        ZStack(alignment: .topLeading) {
            BlurredCoverGround(entry: entry, size: size)
            if sharpCover {
                let frame = Self.sharpFrame(in: size)
                SharpCover(entry: entry, width: frame.width, radius: Self.radius(for: size.width))
                    .offset(x: frame.minX, y: frame.minY)
            }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
    }

    /// Where the sharp cover sits in a frame of `size`.
    static func sharpFrame(in size: CGSize) -> CGRect {
        let height = (size.height * 0.86).rounded()
        let width = (height * 2 / 3).rounded()
        return CGRect(x: ((size.width - width) / 2).rounded(), y: ((size.height - height) / 2).rounded(),
                      width: width, height: height)
    }

    static func radius(for width: CGFloat) -> CGFloat { max(4, (width / 48).rounded()) }
}

/// The blur on its own: the cover enlarged by a quarter past the frame, blurred, darkened to 55%
/// and a touch more saturated.
struct BlurredCoverGround: View {
    let entry: Entry
    let size: CGSize

    var body: some View {
        let width = size.width * 1.24
        let top = CoverFillMode.cropTop(width: size.width, height: size.height, focus: CoverFocus.fallback)
        ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 240)
            .frame(width: width, height: width * 3 / 2)
            .blur(radius: max(4, size.width / 18), opaque: true)
            .saturation(1.1)
            .colorMultiply(Color(white: 0.55))
            .offset(x: -size.width * 0.12, y: top - size.width * 0.12)
            .frame(width: size.width, height: size.height, alignment: .topLeading)
            .clipped()
    }
}

/// The sharp cover on the blur, rounded and ringed.
struct SharpCover: View {
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let width: CGFloat
    let radius: CGFloat

    var body: some View {
        ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: Int(width * 1.5 * scale))
            .frame(width: width, height: (width * 3 / 2).rounded())
            .clipShape(.rect(cornerRadius: radius))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
    }
}

/// A frame with no wide art and no cover: the cover's colour as a plate, the title on it in show
/// type at its foot. The title is the card's own words, said under it, so a screen reader skips
/// this one. A frame too small to read a title in keeps the plate alone.
struct TitlePlate: View {
    let entry: Entry

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            ZStack(alignment: .bottomLeading) {
                CoverPlate(color: entry.plateColor)
                if let type = Self.type(for: size) {
                    Text(entry.showTitle)
                        .font(.showTitle(size: type.size))
                        .fixedLineHeight(type.line)
                        .foregroundStyle(.silk)
                        .lineLimit(2)
                        .multilineTextAlignment(.leading)
                        // The frame keeps its size, so the title grows only so far with the text.
                        .dynamicTypeSize(...DynamicTypeSize.xLarge)
                        .padding(.horizontal, 12)
                        .padding(.bottom, 10)
                        .frame(width: size.width, alignment: .leading)
                }
            }
            .frame(width: size.width, height: size.height)
        }
        .clipped()
        .accessibilityHidden(true)
    }

    /// The title's size in a frame of `size`: the page's title size on a frame taller than a
    /// card, the row's on a card, none where a title would not fit.
    static func type(for size: CGSize) -> (size: CGFloat, line: CGFloat)? {
        if size.width < 120 { return nil }
        return size.height > 150 ? (22, 28) : (16, 24)
    }
}
