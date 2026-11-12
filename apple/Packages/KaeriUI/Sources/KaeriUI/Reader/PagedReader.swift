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
    /// The credit page that was over the leaf fading out, so a turn off it fades it with its leaf.
    @State private var fadingCredit: UInt32?
    @AppStorage(ReaderDefaults.background) private var background = "theme"
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
                slot(index)
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
        // A place the app chose (Back to page, Start over, another device's, a change of mode) is
        // its own move, never the person's jump.
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
        // The chapter's credit pages are decoded ahead, so Show draws one on its first frame.
        .task(id: model.openCredits.map(\.page)) {
            for credit in model.openCredits {
                guard !Task.isCancelled else { return }
                _ = await PageImages.load(credit.file, longestSide: CreditPageView.longest(size, scale))
            }
        }
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

    /// A leaf, with the credit page brought back over it while one shows. The leaf stays under
    /// the credit page, so a step between them crossfades and never fades through empty.
    private func slot(_ index: Int) -> some View {
        ZStack {
            leaf(index)
            if let credit = credit(over: index) {
                CreditPageView(credit: credit, size: size, scale: scale, tone: tone, backdrop: theme.backdrop(background))
                    .id(credit.page)
                    .transition(.opacity)
                    .zIndex(1)
            }
        }
    }

    private func credit(over index: Int) -> CreditPage? {
        if index == current, fading != index { return model.shownCredit }
        if index == fading, let fadingCredit { return model.openCredits.first { $0.page == fadingCredit } }
        return nil
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
        if let step = model.creditStep(forward: forward) {
            creditTurn(step, how: how)
            return
        }
        if forward, model.leaves[current].endPage {
            if let next = model.endNext { model.openChapter(next.id) }
            return
        }
        let target = current + (forward ? 1 : -1)
        guard model.leaves.indices.contains(target) else {
            // Back from the first page goes into the chapter before, at its last page.
            if !forward, current == 0, let previous = model.view?.previous {
                model.openChapter(previous.id, atEnd: true)
            }
            return
        }
        if paper {
            model.shown(target, how: how)
            return
        }
        let old = current
        fadingCredit = model.onCredit
        fading = old
        fade = 0
        model.shown(target, how: how)
        withAnimation(.quick, completionCriteria: .logicallyComplete) {
            fade = 1
        } completion: {
            fading = nil
            fadingCredit = nil
        }
    }

    /// A turn onto, between or off credit pages brought back. On the same leaf the credit page
    /// crossfades over it; onto another leaf it turns as a tap does, the credit page with its leaf.
    private func creditTurn(_ step: ReaderModel.CreditStep, how: TurnHow) {
        let (page, leaf): (UInt32?, Int) = switch step {
        case let .credit(page, leaf): (page, leaf)
        case let .leaf(leaf): (nil, leaf)
        }
        if paper {
            model.showCredit(page, leaf: leaf, how: how)
            return
        }
        if leaf == current {
            withAnimation(page == nil ? .fadeOut : .fadeIn) { model.showCredit(page, leaf: leaf, how: how) }
            return
        }
        fadingCredit = model.onCredit
        fading = current
        fade = 0
        model.showCredit(page, leaf: leaf, how: how)
        withAnimation(.quick, completionCriteria: .logicallyComplete) {
            fade = 1
        } completion: {
            fading = nil
            fadingCredit = nil
        }
    }

    /// Whether a drag the way `forward` stays on the same leaf: between credit pages, or from
    /// one onto its leaf. Such a drag moves a little and lets go into a crossfade.
    private func staysOnLeaf(forward: Bool) -> ReaderModel.CreditStep? {
        guard let step = model.creditStep(forward: forward) else { return nil }
        switch step {
        case let .credit(_, leaf) where leaf == current: return step
        case let .leaf(leaf) where leaf == current: return step
        case .credit: return step
        case .leaf: return nil
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
                if !model.leaves.indices.contains(target) || (endForward && model.endNext == nil) { x /= 4 }
                if staysOnLeaf(forward: forward) != nil { x /= 4 }
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
                if let step = staysOnLeaf(forward: forward) {
                    withAnimation(paper ? nil : .standard) { drag = 0 }
                    if commit { creditTurn(step, how: .swipe) }
                    return
                }
                if commit, endForward {
                    drag = 0
                    if let next = model.endNext { model.openChapter(next.id) }
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
    @Environment(\.readerPaper) private var endPaper
    @Environment(\.readerClose) private var closeReader
    @Environment(Navigator.self) private var navigator
    @Environment(AppStore.self) private var store
    @Environment(\.dynamicTypeSize) private var type

    /// One more swipe opens the next chapter: in the paged reader and a novel, not a strip's foot.
    private var swipes: Bool { held != nil || model.novel != nil || model.view?.mode != .strip }

    // What the page shows: the held chapter's, or the open one's.
    private var view: ReaderView? { held?.view ?? model.view }
    private var end: ReaderEnd? { held == nil ? model.end : held?.end }
    private var read: Bool { held?.read ?? model.read }
    private var readTrackers: [String] { held?.readTrackers ?? model.readTrackers }
    private var medium: Medium { view?.medium ?? .manga }
    /// A strip that could not flow into its next chapter ends here: that chapter, why, and Try
    /// again in place of Continue. Turned to pages, Continue opens it as any chapter opens.
    private var flowFailure: ReaderModel.FlowFailure? {
        held == nil && model.view?.mode == .strip ? model.flowFailure : nil
    }

    private func previewFile(_ id: ChapterId) -> PageFile? {
        if let held { return held.previews[id] }
        return model.previews[id]
    }

    /// Continue, or while the held page waits on a slow first page, a small spinner in its place.
    private func continueLabel(_ id: ChapterId) -> some View {
        pressLabel(L.readerContinue, to: id, tint: theme.onAmber)
    }

    /// A button's words, or the spinner while the chapter it opens is slow to show.
    private func pressLabel(_ text: String, to id: ChapterId, tint: Color) -> some View {
        let waiting = held?.slow == true && held?.to == id
        return Text(text)
            .multilineTextAlignment(.center)
            .opacity(waiting ? 0 : 1)
            .overlay {
                if waiting {
                    ProgressView().controlSize(.small).tint(tint).transition(.opacity)
                }
            }
            .animation(.fadeIn, value: waiting)
    }

    /// Past the extras: "Skip to chapter 17" when the next chapter is one. The extras stay unread.
    @ViewBuilder
    private func skipButton(_ next: ChapterRow, height: CGFloat, wide: Bool) -> some View {
        if let skip = end?.skipTo {
            Button { model.openChapter(skip.id) } label: {
                pressLabel(L.readerEndSkipTo(chapter: skip.number ?? skip.label), to: skip.id, tint: theme.silk)
            }
            .buttonStyle(ReaderPillStyle(theme: theme, height: height, wide: wide))
            .accessibilityHint(end?.skipHint ?? "")
        }
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
        let next = flowFailure?.next ?? ReaderEnd.next(end, view: view)
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
                // The iPad's column is wider, so Continue, the skip and Back fit one 44 pt row.
                .frame(width: phone ? size.width - 56 : (Device.isMac ? 440 : 500), alignment: .leading)
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

    /// The phone's buttons: the swipe line, Continue in amber, the skip past extras under it in
    /// the quiet fill when there is one, and Back to the series.
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
                if flowFailure != nil {
                    Button(L.readerTryAgain) { model.retryFlow() }
                        .buttonStyle(ReaderAmberStyle(theme: theme))
                } else {
                    Button { model.openChapter(next.id) } label: { continueLabel(next.id) }
                        .buttonStyle(ReaderAmberStyle(theme: theme))
                }
                if end?.skipTo != nil {
                    skipButton(next, height: 50, wide: true)
                        .padding(.top, 10)
                }
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
            // The pages skipped at the chapter's end, under what was updated, with Show beside it.
            if held == nil, let chapterId = chapter?.id, !model.endSkipped(chapterId).isEmpty {
                ReaderSkippedLine(count: model.endSkipped(chapterId).count, theme: theme) {
                    // The page only appears over the end page, so it fades in; a strip grows it in.
                    let strip = model.view?.mode == .strip
                    withAnimation(endPaper ? nil : (strip ? .standard : .fadeIn)) { model.showEndCredits(chapterId) }
                }
                .padding(.top, 8)
            }
            if let next {
                Text(L.readerEndNext).font(.ui(13)).foregroundStyle(theme.haze)
                    .padding(.top, phone ? 18 : 22)
                Text(next.title ?? ReaderChrome.chapterName(next.label))
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
                // Why the strip could not go on into it, over Try again.
                if let failure = flowFailure?.failure {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(theme.vermilion)
                            .accessibilityHidden(true)
                        Text(failure.words).foregroundStyle(theme.silk)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .font(.ui(15))
                    .padding(.top, 10)
                    .accessibilityElement(children: .combine)
                }
                // The next chapter's first page once the engine made it, its plate until then. A
                // novel's chapter has no page to show.
                if medium != .novel {
                    preview(next, width: phone ? 150 : 180)
                        .padding(.top, 18)
                }
            } else if let why = end?.why {
                VStack(alignment: .leading, spacing: 6) {
                    Text(title(why, Self.following(view)))
                        .font(.showTitle(size: 22, relativeTo: .title2)).fontWeight(.semibold)
                        .foregroundStyle(theme.silk)
                    if let body = body(why) { Text(body).font(.ui(15)).foregroundStyle(theme.haze) }
                }
                .padding(.top, phone ? 18 : 22)
            } else {
                // Neither the next chapter nor why there is none is known yet: a long list may
                // still be loading, so the page says so rather than caught up.
                HStack(spacing: 8) {
                    ProgressView().controlSize(.small).tint(theme.haze)
                    Text(L.readerChaptersLoading).font(.ui(15)).foregroundStyle(theme.haze)
                }
                .padding(.top, phone ? 18 : 22)
                .transition(.opacity)
            }
            if !phone {
                let stack = VStack(alignment: .leading, spacing: 10) {
                    wideButtons(next, stacked: true)
                }
                .padding(.top, 22)
                if type.isAccessibilitySize {
                    stack
                } else {
                    // One row, Continue first; a row too wide for the column stacks instead.
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 10) { wideButtons(next, stacked: false) }
                            .fixedSize()
                            .padding(.top, 22)
                        stack
                    }
                }
                if next != nil, swipes, !Device.isMac {
                    Text(L.readerEndSwipe).font(.ui(13)).foregroundStyle(theme.haze)
                        .padding(.top, 14)
                }
            }
        }
    }

    /// Continue, the skip and Back on a Mac or iPad: in one row, or stacked, where all three run
    /// the column's width as Continue does.
    @ViewBuilder
    private func wideButtons(_ next: ChapterRow?, stacked: Bool) -> some View {
        let height: CGFloat = Device.isMac ? 28 : 44
        if let next {
            if flowFailure != nil {
                Button { model.retryFlow() } label: {
                    Text(L.readerTryAgain).padding(.horizontal, 18)
                }
                .buttonStyle(ReaderAmberStyle(theme: theme, height: height))
            } else {
                Button { model.openChapter(next.id) } label: {
                    continueLabel(next.id).padding(.horizontal, 18)
                }
                .buttonStyle(ReaderAmberStyle(theme: theme, height: height))
            }
            skipButton(next, height: height, wide: stacked)
        }
        Button(L.readerBackToSeries) { closeReader() }
            .buttonStyle(ReaderPillStyle(theme: theme, height: height, wide: stacked))
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
    private var readLine: String { Self.readLine(read: read, trackers: readTrackers, offline: store.offline) }

    /// Offline the trackers hear of the read only once the device is back online, and the line
    /// says so rather than call them updated.
    static func readLine(read: Bool, trackers: [String], offline: Bool) -> String {
        guard read else { return L.readerEndNotRead }
        guard !trackers.isEmpty else { return L.readerEndRead }
        let names = trackers.formatted(.list(type: .and).locale(AirTime.locale))
        return offline ? L.readerEndTrackerOffline(tracker: names) : L.readerEndTracker(trackers: names)
    }

    /// The chapter the heading names: the next one listed, which offline is one only a source
    /// has; past the last one listed, the next whole number.
    static func following(_ view: ReaderView?) -> String {
        if let next = view?.next { return ReaderChrome.chapterName(next.label) }
        guard let number = view?.chapter.number.flatMap(Double.init) else { return "" }
        return L.readerChapterLine(chapter: String(Int(number) + 1))
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

/// A credit page brought back, drawn over its leaf on the reader's backdrop as one page.
struct CreditPageView: View {
    let credit: CreditPage
    let size: CGSize
    let scale: CGFloat
    let tone: PageTone
    let backdrop: Color
    /// The image is decoded. Until then nothing of the credit page shows, its backdrop included,
    /// so the page under it stays and the two crossfade instead of passing through an empty page.
    @State private var ready: Bool
    @Environment(\.readerPaper) private var paper

    init(credit: CreditPage, size: CGSize, scale: CGFloat, tone: PageTone, backdrop: Color) {
        self.credit = credit
        self.size = size
        self.scale = scale
        self.tone = tone
        self.backdrop = backdrop
        _ready = State(initialValue: PageImages.cached(credit.file, longestSide: Self.longest(size, scale)) != nil)
    }

    static func longest(_ size: CGSize, _ scale: CGFloat) -> CGFloat { max(size.width, size.height) * scale }

    var body: some View {
        ZStack {
            backdrop
            PageImage(file: credit.file, longest: Self.longest(size, scale), tone: tone)
                .accessibilityElement()
                .accessibilityLabel(L.readerCreditPageLabel)
                .accessibilityAddTraits(.isImage)
        }
        .frame(width: size.width, height: size.height)
        .opacity(ready ? 1 : 0)
        .task(id: credit.page) {
            guard !ready else { return }
            _ = await PageImages.load(credit.file, longestSide: Self.longest(size, scale))
            guard !Task.isCancelled else { return }
            withAnimation(paper ? nil : .fadeIn) { ready = true }
        }
    }
}

/// "1 credit page skipped  Show": the end page's line and the strip marker's, 13 point haze with
/// Show underlined in silk.
struct ReaderSkippedLine: View {
    let count: Int
    let theme: ReaderTheme
    let show: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Text(L.readerCreditPagesSkipped(count: count))
                .font(.ui(13)).foregroundStyle(theme.haze)
            Button(L.readerCreditShow, action: show)
                .font(.ui(13, weight: .semibold)).foregroundStyle(theme.silk)
                .underline()
                .buttonStyle(.plain)
                // A 44 point target that keeps the line's own height.
                .padding(.vertical, 12).contentShape(.rect).padding(.vertical, -12)
                .pointerHover()
                .accessibilityHint(L.readerCreditShowHint)
        }
        .accessibilityElement(children: .contain)
        // As the paged credit line: it stops growing at accessibility1.
        .dynamicTypeSize(...DynamicTypeSize.accessibility1)
    }
}
