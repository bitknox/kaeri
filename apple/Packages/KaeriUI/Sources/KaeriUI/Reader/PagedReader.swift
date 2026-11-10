// Pages one at a time, or two as a spread. Swipes follow the finger, taps and keys crossfade,
// and Paper mode turns instantly with nothing in between.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PagedReader: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let size: CGSize
    let scale: CGFloat
    let tone: PageTone
    let paper: Bool
    let onTap: (Double) -> Void
    @State private var drag: CGFloat = 0
    @State private var fading: Int?
    @State private var fade: Double = 1
    @State private var settling = false
    @State private var zoom = PageZoom()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var rtl: Bool { model.view?.direction == .rightToLeft }
    /// Which way a drag goes to move forward: right in right-to-left, left otherwise.
    private var sign: CGFloat { rtl ? 1 : -1 }
    private var current: Int { min(model.current, max(model.leaves.count - 1, 0)) }

    var body: some View {
        ZStack {
            // The neighbours stay mounted off screen, so a drag starts with its page already there,
            // and the page a tap turn leaves is the same view held under the one that fades in.
            ForEach(mounted, id: \.self) { index in
                leaf(index)
                    .zoomed(zoom, on: index == current)
                    .offset(x: offset(of: index))
                    .opacity(index == current && fading != nil ? fade : 1)
                    .zIndex(index == current ? 1 : 0)
                    .accessibilityHidden(index != current)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipped()
        .contentShape(.rect)
        .gesture(swipe)
        #if os(iOS)
        // A double tap zooms, so a single tap waits for it not to be one.
        .simultaneousGesture(
            SpatialTapGesture(count: 2).onEnded { zoom.doubleTap(at: $0.location, in: size, motion: !reduceMotion && !paper) }
                .exclusively(before: SpatialTapGesture().onEnded { tap($0.location.x / max(size.width, 1)) }))
        .simultaneousGesture(
            MagnifyGesture()
                .onChanged { zoom.pinch($0.magnification, at: $0.startLocation, in: size) }
                .onEnded { _ in zoom.pinchEnded(in: size, motion: !reduceMotion && !paper) })
        #else
        .simultaneousGesture(SpatialTapGesture().onEnded { tap($0.location.x / max(size.width, 1)) })
        #endif
        .onAppear { model.position = (current, max(model.leaves.count, 1)) }
        .onChange(of: model.current) { _, _ in
            model.position = (current, max(model.leaves.count, 1))
            zoom.reset()
        }
        .onChange(of: model.leaves.count) { _, _ in
            model.position = (current, max(model.leaves.count, 1))
            zoom.reset()
        }
        .onChange(of: model.goTo?.tick) { _, _ in
            if let to = model.goTo { model.shown(to.leaf, how: to.how) }
        }
        .onChange(of: model.jump?.tick) { _, _ in
            if let to = model.jump?.index, model.leaves.indices.contains(to) { model.shown(to, how: .jump) }
        }
        .onChange(of: model.turnRequest) { _, request in
            guard let request else { return }
            turn(forward: request.forward, how: .key)
        }
        .accessibilityScrollAction { edge in turn(forward: edge == .trailing || edge == .bottom, how: .turn) }
    }

    private var mounted: [Int] {
        var all = Set([current])
        if model.leaves.indices.contains(current + 1) { all.insert(current + 1) }
        if model.leaves.indices.contains(current - 1) { all.insert(current - 1) }
        if let fading, model.leaves.indices.contains(fading) { all.insert(fading) }
        return all.sorted()
    }

    private func offset(of index: Int) -> CGFloat {
        if index == current { return drag }
        if index == fading { return 0 }
        return drag + (index > current ? -sign : sign) * size.width
    }

    @ViewBuilder private func leaf(_ index: Int) -> some View {
        if model.leaves.indices.contains(index) {
            let item = model.leaves[index]
            if item.endPage {
                ReaderEndPage(model: model, theme: theme, size: size)
            } else {
                LeafView(
                    model: model, leaf: item, size: size, theme: theme, tone: tone,
                    // A zoomed page is decoded again at its zoom once the fingers are up, so it stays sharp.
                    scale: scale * (index == current ? zoom.detail : 1),
                    fitWidth: model.choice(.fit) == "width")
                    .background(Color.clear)
            }
        } else {
            Color.clear
        }
    }

    // MARK: Turning

    private func tap(_ x: Double) {
        let layout = model.choice(.tapLayout) ?? "edges"
        // While zoomed the zones are off: a tap anywhere only brings the chrome or puts it away.
        if zoom.zoomed { onTap(0.5); return }
        if model.chromeShown || layout == "off" { onTap(x); return }
        // Edges: the outer thirds turn. Halves: the outer 40 percent turn, and the middle band
        // still brings the chrome up.
        let edge = layout == "sides" ? 0.4 : 0.33
        if x < edge || x > 1 - edge {
            let leftSide = x < 0.5
            // Left is forward when the book runs right to left.
            turn(forward: leftSide == rtl, how: .turn)
        } else {
            onTap(x)
        }
    }

    private func turn(forward: Bool, how: TurnHow) {
        guard model.leaves.indices.contains(current) else { return }
        model.hideChrome()
        if forward, model.leaves[current].endPage {
            if let next = model.view?.next { model.openChapter(next.id) }
            return
        }
        let target = current + (forward ? 1 : -1)
        guard model.leaves.indices.contains(target) else { return }
        if paper {
            model.shown(target, how: how)
            return
        }
        let old = current
        fading = old
        fade = 0
        model.shown(target, how: how)
        withAnimation(.quick, completionCriteria: .logicallyComplete) {
            fade = 1
        } completion: {
            fading = nil
        }
    }

    private var swipe: some Gesture {
        DragGesture(minimumDistance: 14)
            .onChanged { value in
                guard !settling else { return }
                // A zoomed page pans and never turns; a pinch's fingers do neither.
                if zoom.zoomed || zoom.pinching {
                    zoom.drag(value.translation, in: size)
                    return
                }
                var x = value.translation.width
                let forward = x * sign > 0
                let target = current + (forward ? 1 : -1)
                let endForward = forward && model.leaves.indices.contains(current) && model.leaves[current].endPage
                if !model.leaves.indices.contains(target) || (endForward && model.view?.next == nil) { x /= 4 }
                drag = x
            }
            .onEnded { value in
                guard !settling else { return }
                if zoom.zoomed || zoom.pinching {
                    zoom.dragEnded()
                    withAnimation(paper ? nil : .standard) { drag = 0 }
                    return
                }
                let x = value.translation.width
                let flick = abs(value.velocity.width) > 450 && value.velocity.width * x > 0
                let commit = abs(x) > size.width * 0.25 || flick
                let forward = x * sign > 0
                let target = current + (forward ? 1 : -1)
                let endForward = forward && model.leaves.indices.contains(current) && model.leaves[current].endPage
                if commit, endForward {
                    drag = 0
                    if let next = model.view?.next { model.openChapter(next.id) }
                    return
                }
                guard commit, model.leaves.indices.contains(target) else {
                    withAnimation(paper ? nil : .standard) { drag = 0 }
                    return
                }
                if paper {
                    drag = 0
                    model.shown(target, how: .swipe)
                    return
                }
                settling = true
                withAnimation(.standard, completionCriteria: .logicallyComplete) {
                    drag = sign * (forward ? 1 : -1) * size.width
                } completion: {
                    var transaction = Transaction()
                    transaction.disablesAnimations = true
                    withTransaction(transaction) {
                        model.shown(target, how: .swipe)
                        drag = 0
                        settling = false
                    }
                }
            }
    }
}

// MARK: End page

/// The page after the last: 28 pt margins, the next chapter, and the one amber Continue.
struct ReaderEndPage: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let size: CGSize
    /// The end page as it was when the chapter was left, held over the next one while it opens.
    var held: ReaderModel.HeldEnd?
    @Environment(\.readerClose) private var closeReader
    @Environment(Navigator.self) private var navigator
    @Environment(AppStore.self) private var store
    @Environment(\.dynamicTypeSize) private var type

    /// One more swipe opens the next chapter: in the paged reader, not a strip's foot or a novel.
    private var swipes: Bool { held != nil || (model.novel == nil && model.view?.mode != .strip) }

    // What the page shows: the held chapter's, or the open one's.
    private var view: ReaderView? { held?.view ?? model.view }
    private var end: ReaderEnd? { held == nil ? model.end : held?.end }
    private var read: Bool { held?.read ?? model.read }
    private var readTrackers: [String] { held?.readTrackers ?? model.readTrackers }
    private var medium: Medium { view?.medium ?? .manga }

    private func previewFile(_ id: ChapterId) -> PageFile? {
        if let held { return held.previews[id] }
        return model.previews[id]
    }

    /// Continue, or while the held page waits on a slow first page, a small spinner in its place.
    private func continueLabel(_ id: ChapterId) -> some View {
        let waiting = held?.slow == true && held?.to == id
        return Text(L.readerContinue)
            .opacity(waiting ? 0 : 1)
            .overlay {
                if waiting {
                    ProgressView().controlSize(.small).tint(theme.onAmber).transition(.opacity)
                }
            }
            .animation(.fadeIn, value: waiting)
    }

    var body: some View {
        if view?.demoCredit != nil {
            demoEnd
        } else {
            standard
        }
    }

    /// The end of the demo: a card with the work's cover, what it was, the way to add a source,
    /// and back to page 1. The reader's close button leads back to the series.
    private var demoEnd: some View {
        VStack(spacing: 0) {
            if let cover = view?.demoCredit?.cover {
                BookCover(url: cover, width: Device.isMac ? 40 : 48)
                    .padding(.bottom, 14)
            }
            Text(L.readerDemoEnd)
                .font(.ui(Device.isMac ? 15 : 17, weight: .semibold))
                .foregroundStyle(theme.silk)
            Text(L.readerDemoEndLine)
                .font(.ui(Device.isMac ? 12 : 14))
                .foregroundStyle(theme.haze)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 4)
            Button(L.readerDemoAddSource) {
                closeReader()
                navigator.addBookSource = .choose
            }
            .buttonStyle(ReaderAmberStyle(theme: theme, height: Device.isMac ? 26 : 40))
            .padding(.top, Device.isMac ? 16 : 18)
            if Device.isMac {
                Button(L.readerDemoAgain) { model.requestJump(0) }
                    .buttonStyle(ReaderPillStyle(theme: theme, height: 26, wide: true, fill: theme.page.opacity(0.7)))
                    .padding(.top, 8)
            } else {
                Button(L.readerDemoAgain) { model.requestJump(0) }
                    .font(.ui(15, weight: .semibold))
                    .foregroundStyle(theme.silk)
                    .frame(minHeight: 44)
                    .buttonStyle(.plain)
                    .padding(.top, 4)
            }
        }
        .padding(EdgeInsets(top: 20, leading: 18, bottom: Device.isMac ? 18 : 10, trailing: 18))
        .frame(maxWidth: Device.isMac ? 300 : 340)
        .background(theme.high, in: .rect(cornerRadius: 14))
        .padding(.horizontal, 16)
        .frame(width: size.width, height: size.height)
        .background(theme.page)
    }

    /// The page between chapters, as the board lays it: what was read and where it went, the
    /// next chapter with its facts and its first page, and Continue. On a phone the buttons are
    /// pinned at the foot; wider, the column is centred with the buttons under it.
    private var standard: some View {
        let next = end?.next ?? view?.next
        let phone = size.width < 600
        // At the largest sizes nothing is pinned: the buttons follow the column in the scroll.
        let pinned = phone && !type.isAccessibilitySize
        return ZStack(alignment: .topLeading) {
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    column(next, phone: phone)
                    if phone && !pinned {
                        buttons(next)
                            .padding(.top, 28)
                    }
                }
                .frame(width: phone ? size.width - 56 : 440, alignment: .leading)
                .padding(.top, phone ? 100 : (Device.isMac ? 120 : 150))
                .padding(.bottom, pinned ? 230 : 40)
                .frame(maxWidth: .infinity, alignment: phone ? .leading : .center)
                .padding(.leading, phone ? 28 : 0)
            }
            .scrollBounceBehavior(.basedOnSize)
            .scrollIndicators(type.isAccessibilitySize ? .visible : .hidden)
            if pinned {
                VStack(spacing: 0) {
                    Spacer(minLength: 0)
                    buttons(next)
                }
                .padding(.horizontal, 28)
                .padding(.bottom, 23)
            }
        }
        .frame(width: size.width, height: size.height)
        .background(theme.page)
    }

    /// The phone's buttons: the swipe line, Continue in amber and Back to the series under it.
    @ViewBuilder
    private func buttons(_ next: ChapterRow?) -> some View {
        VStack(spacing: 0) {
            if let next {
                if swipes {
                    Text(L.readerEndSwipe).font(.ui(13)).foregroundStyle(theme.haze)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: .infinity)
                        .padding(.bottom, 10)
                }
                Button { model.openChapter(next.id) } label: { continueLabel(next.id) }
                    .buttonStyle(ReaderAmberStyle(theme: theme))
                Button(L.readerBackToSeries) { closeReader() }
                    .font(.ui(15, weight: .semibold))
                    .foregroundStyle(theme.silk)
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .contentShape(.rect)
                    .buttonStyle(.plain)
                    .padding(.top, 5)
            } else {
                Button(L.readerBackToSeries) { closeReader() }
                    .buttonStyle(ReaderAmberStyle(theme: theme))
            }
        }
    }

    @ViewBuilder
    private func column(_ next: ChapterRow?, phone: Bool) -> some View {
        let chapter = view?.chapter
        VStack(alignment: .leading, spacing: 0) {
            Text(L.readerEndOfChapterNumber(chapter: chapter.map { $0.number ?? $0.label } ?? ""))
                .font(.ui(15)).foregroundStyle(theme.haze)
            Text(readLine).font(.ui(13)).foregroundStyle(theme.haze)
                .padding(.top, 4)
            if let work = view?.work, let chapterId = chapter?.id,
               store.serverProgress.holds(work: work, chapter: chapterId) {
                ReaderServerLine(line: store.serverProgress.lines[chapterId], theme: theme) {
                    store.serverProgress.retry(chapter: chapterId)
                }
                .padding(.top, 2)
            }
            if let next {
                Text(L.readerEndNext).font(.ui(13)).foregroundStyle(theme.haze)
                    .padding(.top, phone ? 18 : 22)
                Text(next.title ?? next.label)
                    .font(.showTitle(size: 30, relativeTo: .title))
                    .fontWeight(.bold)
                    .foregroundStyle(theme.silk)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 4)
                let facts = Self.facts(next, end: end)
                if !facts.isEmpty {
                    Text(facts).font(.ui(14)).foregroundStyle(theme.haze)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 4)
                }
                // The next chapter's first page once the engine made it, its plate until then. A
                // novel's chapter has no page to show.
                if medium != .novel {
                    preview(next, width: phone ? 150 : 180)
                        .padding(.top, 18)
                }
            } else if let why = end?.why {
                VStack(alignment: .leading, spacing: 6) {
                    Text(title(why, chapter?.label ?? ""))
                        .font(.showTitle(size: 22, relativeTo: .title2)).fontWeight(.semibold)
                        .foregroundStyle(theme.silk)
                    if let body = body(why) { Text(body).font(.ui(15)).foregroundStyle(theme.haze) }
                }
                .padding(.top, phone ? 18 : 22)
            }
            if !phone {
                let row = Group {
                    if let next {
                        Button { model.openChapter(next.id) } label: {
                            continueLabel(next.id).padding(.horizontal, 18)
                        }
                        .buttonStyle(ReaderAmberStyle(theme: theme, height: Device.isMac ? 28 : 44))
                    }
                    Button(L.readerBackToSeries) { closeReader() }
                        .buttonStyle(ReaderPillStyle(theme: theme, height: Device.isMac ? 28 : 44))
                }
                if type.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: 10) { row }.padding(.top, 22)
                } else {
                    HStack(spacing: 10) { row }.fixedSize().padding(.top, 22)
                }
                if next != nil, swipes, !Device.isMac {
                    Text(L.readerEndSwipe).font(.ui(13)).foregroundStyle(theme.haze)
                        .padding(.top, 14)
                }
            }
        }
    }

    private func preview(_ next: ChapterRow, width: CGFloat) -> some View {
        let height = (width * 1.42).rounded()
        return ZStack {
            theme.high
            // With no picture of its first page yet, the plate carries the chapter's number.
            Text(verbatim: next.number ?? next.label)
                .font(.ui(15)).monospacedDigit().foregroundStyle(theme.haze)
                .lineLimit(2).multilineTextAlignment(.center).padding(8)
            if let file = previewFile(next.id) {
                PageImage(file: file, longest: height * 3, fill: true)
            }
        }
        .frame(width: width, height: height)
        .clipShape(.rect(cornerRadius: 4))
        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(theme.haze.opacity(0.18), lineWidth: 1))
        .shadow(color: theme.shadow, radius: 15, y: 10)
        .accessibilityHidden(true)
    }

    /// "Chapter 13 · 34 pages · from books.home.example": the chapter's number when it has a
    /// title of its own, its length, and the source it came from.
    static func facts(_ next: ChapterRow, end: ReaderEnd?) -> String {
        let chapter = next.title.map { _ in next.number ?? next.label }
        let pages = next.pages.flatMap { $0 > 0 ? L.readerEndPages(count: Int($0)) : nil }
        let facts: String? = switch (chapter, pages) {
        case (let chapter?, let pages?): L.readerEndFacts(chapter: chapter, facts: pages)
        case (let chapter?, nil): L.readerChapterLine(chapter: chapter)
        case (nil, let pages): pages
        }
        let source = end?.next?.id == next.id ? end?.nextSource : nil
        switch (facts, source) {
        case (let facts?, let source?): return L.readerDividerFactsSource(facts: facts, source: source)
        case (nil, let source?): return L.readerEndFrom(source: source)
        case (let facts, nil): return facts ?? ""
        }
    }

    /// "Read · AniList updated", or Read alone when no tracker took it.
    private var readLine: String {
        guard read else { return L.readerEndNotRead }
        guard !readTrackers.isEmpty else { return L.readerEndRead }
        return L.readerEndTracker(trackers: readTrackers.formatted(.list(type: .and).locale(AirTime.locale)))
    }

    private func title(_ why: NoNext, _ chapter: String) -> String {
        switch why {
        case .notOutYet: L.readerEndNotOut(chapter: chapter)
        case .caughtUp: L.readerEndCaughtUp
        case .finished: L.readerEndFinished
        case .noSourceHasIt: L.readerEndNoSource(chapter: chapter)
        case .offline: L.readerEndOffline(chapter: chapter)
        }
    }

    private func body(_ why: NoNext) -> String? {
        switch why {
        case .notOutYet: L.readerEndNotOutBody
        case .caughtUp: L.readerEndCaughtUpBody
        case .finished: L.readerEndFinishedBody
        case .noSourceHasIt: nil
        case .offline: L.readerEndOfflineBody
        }
    }
}

// MARK: The server line

/// Did my server get it: one line under the tracker line, its two lines of room held from the
/// first frame so Next and Continue never move. Each change crossfades in place (0.3 s reveal);
/// Reduce Motion keeps the crossfade, which moves nothing.
struct ReaderServerLine: View {
    let line: ServerLine?
    let theme: ReaderTheme
    let retry: () -> Void
    @ScaledMetric(relativeTo: .footnote) private var room: CGFloat = 36

    var body: some View {
        ZStack(alignment: .topLeading) {
            if let line {
                content(line)
                    .id(Self.key(line))
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, minHeight: room, alignment: .topLeading)
        .animation(.reveal, value: line.map(Self.key))
        .environment(\.openURL, OpenURLAction { _ in
            retry()
            return .handled
        })
    }

    private func content(_ line: ServerLine) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            glyph(line)
            words(line)
                .font(.ui(13))
                .foregroundStyle(Self.failed(line) ? theme.silk : theme.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L.readerTryAgain) { if line.canRetry { retry() } }
    }

    @ViewBuilder
    private func glyph(_ line: ServerLine) -> some View {
        switch line {
        case .saved: EmptyView()
        case .sending:
            ProgressView().controlSize(.mini).tint(theme.haze)
                .alignmentGuide(.firstTextBaseline) { $0[VerticalAlignment.center] + 4 }
        case .offline: Image(systemName: "wifi.slash").font(.ui(12)).foregroundStyle(theme.haze)
        case .retrying: Image(systemName: "arrow.triangle.2.circlepath").font(.ui(12)).foregroundStyle(theme.haze)
        case .failed: Image(systemName: "exclamationmark.triangle.fill").font(.ui(12)).foregroundStyle(theme.vermilion)
        }
    }

    /// The words, with Try again as a link after them where the line can be tried again.
    private func words(_ line: ServerLine) -> Text {
        let said = Text(ServerProgressWords.line(line))
        guard line.canRetry, let url = URL(string: "kaeri-retry:progress") else { return said } // i18n-skip: never shown
        var link = AttributedString(L.readerTryAgain)
        link.link = url
        link.underlineStyle = .single
        link.foregroundColor = theme.silk
        return said + Text(" ") + Text(link)
    }

    private static func failed(_ line: ServerLine) -> Bool {
        if case .failed = line { true } else { false }
    }

    /// One key per kind of line, so a change of kind crossfades and a same kind stays.
    static func key(_ line: ServerLine) -> String {
        switch line {
        case .saved: "saved" // i18n-skip: an animation key
        case .sending: "sending" // i18n-skip: an animation key
        case .offline: "offline" // i18n-skip: an animation key
        case .retrying: "retrying" // i18n-skip: an animation key
        case .failed: "failed" // i18n-skip: an animation key
        }
    }
}
