// A page on screen: its image, the plate it fades in over, and the error in its place.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// Paper mode and Greyscale pages, as they touch a page's pixels.
struct PageTone: Equatable {
    var dim = false
    var grey = false
}

struct PageImage: View {
    let file: PageFile
    let longest: CGFloat
    var tone = PageTone()
    var fill = false
    /// Called once the image is on screen: after its fade, or at once when it drew on its first
    /// frame.
    var onPaint: (() -> Void)?
    @State private var image: CGImage?
    @Environment(\.readerPaper) private var paper
    @Environment(\.readerHeld) private var held

    init(file: PageFile, longest: CGFloat, tone: PageTone = PageTone(), fill: Bool = false, onPaint: (() -> Void)? = nil) {
        self.file = file
        self.longest = longest
        self.tone = tone
        self.fill = fill
        self.onPaint = onPaint
        // A page that is already decoded draws on the first frame, with no fade.
        _image = State(initialValue: PageImages.cached(file, longestSide: longest))
    }

    var body: some View {
        ZStack {
            if let image {
                Image(decorative: image, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: fill ? .fill : .fit)
                    .saturation(tone.grey ? 0 : 1)
                    // Dim the page's white instead of tinting it.
                    .brightness(tone.dim ? -0.07 : 0)
                    .transition(.opacity)
            }
        }
        .onAppear {
            // Back on screen after its pixels were let go: the cache's copy draws at once.
            if image == nil { image = PageImages.cached(file, longestSide: longest) }
            if image != nil { onPaint?() }
        }
        // Off screen, the page lets its pixels go: a long strip keeps every row it has shown,
        // and each holding its own image would hold the whole run. The cache keeps the recent
        // ones, up to its limit, for coming back.
        .onDisappear { image = nil }
        // The crop is in the id: crop borders on and off send the same original with another crop.
        .task(id: "\(file.path)|\(file.crop.map { "\($0.x),\($0.y),\($0.width),\($0.height)" } ?? "")|\(Int(longest))") {
            guard let loaded = await PageImages.load(file, longestSide: longest), !Task.isCancelled else { return }
            if image === loaded { return }
            // A page that arrives late fades in over its plate; Reduce Motion keeps the fade,
            // which moves nothing. Paper mode draws it at once, and so does a page under the held
            // end page, which fades away over it instead.
            if image == nil, !paper, !held {
                withAnimation(.fadeIn, completionCriteria: .logicallyComplete) {
                    image = loaded
                } completion: {
                    onPaint?()
                }
            } else {
                image = loaded
                onPaint?()
            }
        }
    }
}

/// The Raised plate that holds a page's place with its number, "Page 23", or "Image 23" for a
/// strip image. Nothing shows for 600 ms, and from 4 s a line under the number says it is
/// loading. A chapter that is opening shows it at once: it is the place the first page comes
/// into. A strip image's words sit in the middle of the part of it on screen, so they stay in
/// view however tall the image is and wherever the strip is scrolled to.
struct PagePlate: View {
    let page: Int
    let theme: ReaderTheme
    let image: Bool
    @State private var shown: Bool
    @State private var line = false
    @State private var words: CGFloat = 0

    init(page: Int, theme: ReaderTheme, atOnce: Bool = false, image: Bool = false) {
        self.page = page
        self.theme = theme
        self.image = image
        _shown = State(initialValue: atOnce)
    }

    /// How far a strip plate's words move from its middle so they sit in the middle of the part
    /// of the plate on screen, kept inside the plate. `visible` is that part in the plate's own
    /// coordinates; with none on screen the words stay in the middle.
    nonisolated static func wordsShift(plate: CGFloat, visible: ClosedRange<CGFloat>?, words: CGFloat) -> CGFloat {
        guard let visible, visible.upperBound > visible.lowerBound, plate > words else { return 0 }
        let middle = (visible.lowerBound + visible.upperBound) / 2
        return min(max(middle, words / 2), plate - words / 2) - plate / 2
    }

    /// The part of the plate inside its scroll view, in the plate's coordinates.
    private nonisolated static func onScreen(_ proxy: GeometryProxy) -> ClosedRange<CGFloat>? {
        guard let bounds = proxy.bounds(of: .scrollView) else { return nil }
        let top = max(bounds.minY, 0)
        let bottom = min(bounds.maxY, proxy.size.height)
        return top < bottom ? top...bottom : nil
    }

    var body: some View {
        ZStack {
            if shown {
                Rectangle().fill(theme.raised)
                VStack(spacing: 6) {
                    Text(image ? L.readerImagePlate(image: page) : L.readerPagePlate(page: page))
                        .font(.ui(15)).monospacedDigit().foregroundStyle(theme.haze)
                    if line {
                        Text(image ? L.readerImageLoading(image: page) : L.readerPageLoading(page: page))
                            .font(.ui(13)).foregroundStyle(theme.haze)
                            .transition(.opacity)
                    }
                }
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { words = $0 }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .visualEffect { [image, words] content, proxy in
                    content.offset(y: image ? Self.wordsShift(
                        plate: proxy.size.height, visible: Self.onScreen(proxy), words: words) : 0)
                }
                .transition(.opacity)
            }
        }
        .accessibilityHidden(true)
        .task {
            if !shown {
                try? await Task.sleep(for: .milliseconds(600))
                guard !Task.isCancelled else { return }
                withAnimation(.fadeIn) { shown = true }
                try? await Task.sleep(for: .milliseconds(3400))
            } else {
                try? await Task.sleep(for: .seconds(4))
            }
            guard !Task.isCancelled else { return }
            withAnimation(.fadeIn) { line = true }
        }
    }
}

/// A page that failed: left-aligned in the page's place, vermilion only on the glyph, Try again
/// as the one action, and turning past it works. In a strip it names the image ("Image 4",
/// "Couldn't load") and leaves out the swipe hint, since scrolling goes on past it.
struct PageFailed: View {
    let page: Int
    let failure: ReadingFailure
    let theme: ReaderTheme
    var strip = false
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 22)).foregroundStyle(theme.vermilion)
                .accessibilityHidden(true)
            Text(strip ? L.readerImagePlate(image: page) : L.readerPageFailed(page: page))
                .font(.ui(17, weight: .semibold)).foregroundStyle(theme.silk)
            if strip {
                Text(L.readerImageFailed).font(.ui(15)).foregroundStyle(theme.haze)
            }
            Text(failure.words).font(.ui(15)).foregroundStyle(theme.haze)
            Button(L.readerTryAgain, action: retry)
                .buttonStyle(ReaderPillStyle(theme: theme))
                .padding(.top, 4)
            if !strip {
                Text(L.readerPageFailedHint).font(.ui(13)).foregroundStyle(theme.haze)
            }
        }
        .frame(maxWidth: 320, alignment: .leading)
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(.top, 120)
        .background(theme.raised)
    }
}

/// A quiet pill, the reader's secondary button.
struct ReaderPillStyle: ButtonStyle {
    let theme: ReaderTheme
    var height: CGFloat = 40
    /// Across its row, as the amber button above it.
    var wide = false
    /// On a card that is already the raised tone, the page's tone.
    var fill: Color?

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.ui(height < 32 ? 12 : 15, weight: .semibold))
            .foregroundStyle(theme.silk)
            .padding(.horizontal, 18)
            .frame(maxWidth: wide ? .infinity : nil, minHeight: height)
            .background((fill ?? theme.high).opacity(configuration.isPressed ? 0.8 : 1), in: .capsule)
            .contentShape(.capsule)
    }
}

/// The reader's one amber action.
struct ReaderAmberStyle: ButtonStyle {
    let theme: ReaderTheme
    var height: CGFloat = 50

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.ui(height < 32 ? 12 : height < 50 ? 15 : 17, weight: .semibold))
            .foregroundStyle(theme.onAmber)
            .frame(maxWidth: .infinity, minHeight: height)
            .background(theme.amber.opacity(configuration.isPressed ? 0.85 : 1), in: .capsule)
            .contentShape(.capsule)
    }
}

/// One leaf: its parts laid out where the engine put them in a box of `size`.
struct LeafView: View {
    @Bindable var model: ReaderModel
    let leaf: Leaf
    let size: CGSize
    let theme: ReaderTheme
    let tone: PageTone
    let scale: CGFloat
    var fitWidth = false

    var body: some View {
        let chapter = model.view?.chapter.id ?? ""
        ZStack(alignment: .topLeading) {
            ForEach(0..<leaf.parts.count, id: \.self) { index in
                partView(leaf.parts[index], chapter: chapter)
            }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
    }

    private func partView(_ part: LeafPart, chapter: String) -> some View {
        let w = CGFloat(part.place.width) * size.width
        let h = CGFloat(part.place.height) * size.height
        let x = CGFloat(part.place.x) * size.width
        let y = CGFloat(part.place.y) * size.height
        let double = leaf.parts.count > 1
        let align: Alignment = double ? (part.place.x < 0.5 ? .trailing : .leading) : .center
        return PartView(
            model: model, chapter: chapter, part: part, box: CGSize(width: w, height: h), theme: theme,
            tone: tone, scale: scale, align: align, fitWidth: fitWidth
        )
        .frame(width: w, height: h)
        .offset(x: x, y: y)
    }
}

struct PartView: View {
    @Bindable var model: ReaderModel
    let chapter: String
    let part: LeafPart
    let box: CGSize
    let theme: ReaderTheme
    let tone: PageTone
    let scale: CGFloat
    var align: Alignment = .center
    var fitWidth = false
    @State private var painted = false

    /// The chapter has only just opened: its plate is the one the opening screen showed.
    private var fresh: Bool {
        model.openedAt.map { ContinuousClock.now - $0 < .seconds(3) } ?? false
    }

    var body: some View {
        let number = Int(part.page) + 1
        ZStack(alignment: align) {
            // The plate holds the place until the image is on screen, and the image fades in over it.
            if !painted, model.failure(of: part, chapter: chapter) == nil {
                PagePlate(page: number, theme: theme, atOnce: fresh)
            }
            if let file = model.file(of: part, chapter: chapter) {
                // Fit width draws the page as wide as the box however tall that makes it, so it
                // is decoded for that height, not the box's.
                let shown = fitWidth && !part.rotate ? PageImages.longest(file, width: box.width) : max(box.width, box.height)
                let image = PageImage(file: file, longest: shown * scale, tone: tone, onPaint: {
                    painted = true
                    model.pagePainted(chapter: chapter, page: part.page)
                })
                    .accessibilityElement()
                    .accessibilityLabel(L.readerPageLabel(page: number, total: max(model.pageCount, number), title: model.view?.title ?? ""))
                    .accessibilityAddTraits(.isImage)
                if part.rotate {
                    image.frame(width: box.height, height: box.width).rotationEffect(.degrees(90))
                        .frame(width: box.width, height: box.height)
                } else if fitWidth {
                    ScrollView(.vertical, showsIndicators: false) {
                        image.frame(width: box.width)
                    }
                } else {
                    image.frame(width: box.width, height: box.height, alignment: align)
                }
            } else if let failure = model.failure(of: part, chapter: chapter) {
                PageFailed(page: number, failure: failure, theme: theme) { model.retry(page: part.page) }
            }
        }
        .frame(width: box.width, height: box.height, alignment: align)
        .clipped()
    }
}
