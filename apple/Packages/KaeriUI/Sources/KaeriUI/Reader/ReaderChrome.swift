// The reader's chrome: the group at the bottom with the scrubber and Back to page, and the notice
// capsule. All of it is glass over the page.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct ReaderChrome: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    let onClose: () -> Void
    /// The group's height, for a novel's notice to clear it.
    @Binding var groupHeight: CGFloat
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.dynamicTypeSize) private var type
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.readerPaper) private var paper
    @State private var scrub: Int?
    #if os(macOS)
    @State private var fullScreen = false
    #endif

    private var wide: Bool { mac || sizeClass == .regular }
    private var rtl: Bool { model.view?.direction == .rightToLeft && model.view?.mode != .strip && model.medium == .manga }
    /// At the accessibility sizes the title and its line take a row of their own.
    private var big: Bool { type.isAccessibilitySize }

    var body: some View {
        VStack {
            Spacer()
            group
                .frame(maxWidth: wide ? 640 : .infinity)
                .padding(.horizontal, wide ? 0 : 12)
                .padding(.bottom, Self.bottom(mac: mac, wide: wide))
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { groupHeight = $0 }
        }
        .onChange(of: model.shotScrub) { _, held in if let held { scrub = held } }
        .onChange(of: model.backToPage) { _, page in
            if let page, model.chromeShown { AccessibilityNotification.Announcement(L.readerBackToPage(page: page)).post() }
        }
        .opacity(model.chromeShown ? 1 : 0)
        .allowsHitTesting(model.chromeShown)
        .accessibilityHidden(!model.chromeShown)
    }

    /// The group's distance from the bottom of the screen.
    static func bottom(mac: Bool, wide: Bool) -> CGFloat { mac ? 20 : (wide ? 24 : 28) }

    /// The board's group: the scrubber with the page at its start and the count at its end, then
    /// Close, the title over its line, Chapters and Aa. On the Mac the window's buttons stand in
    /// for Close, and Full screen ends the row.
    private var group: some View {
        VStack(spacing: 0) {
            if let page = model.backToPage {
                backTo(page)
                    .padding(.horizontal, 12)
                    .padding(.top, 8)
                    .transition(.opacity)
            }
            scrubberRow
                .padding(.horizontal, 18)
                .padding(.top, 6)
            if big {
                titleStack
                    .padding(.horizontal, 18)
                    .padding(.top, 6)
            }
            HStack(spacing: 4) {
                if mac {
                    // Back to the window as it was before the book opened, as Esc goes.
                    button("chevron.backward", L.actionBack, action: onClose)
                    chaptersButton
                } else {
                    button("xmark", L.readerClose, action: onClose).accessibilitySortPriority(5)
                }
                if big { Spacer(minLength: 0) } else { titleStack }
                if !mac { chaptersButton }
                settingsButton
                #if os(macOS)
                button(
                    fullScreen ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                    fullScreen ? L.playerExitFullScreen : L.playerEnterFullScreen
                ) {
                    NSApp.keyWindow?.toggleFullScreen(nil)
                }
                .onReceive(NotificationCenter.default.publisher(for: NSWindow.didEnterFullScreenNotification)) { _ in fullScreen = true }
                .onReceive(NotificationCenter.default.publisher(for: NSWindow.didExitFullScreenNotification)) { _ in fullScreen = false }
                .onAppear { fullScreen = NSApp.keyWindow?.styleMask.contains(.fullScreen) ?? false }
                #endif
            }
            .padding(.horizontal, 6)
            .frame(minHeight: 44)
            .padding(.bottom, 6)
        }
        .glassEffect(.regular, in: .rect(cornerRadius: 30))
        // The group grows upwards, so the scrubber under the finger stays where it is.
        .animation(paper ? nil : (reduceMotion ? .fadeIn : .standard), value: model.backToPage == nil)
    }

    /// Back to page, offered after the person's own jump of more than two pages: a quiet
    /// button at the leading end above the scrubber. It goes at the next turn.
    private func backTo(_ page: Int) -> some View {
        HStack {
            Button { model.back() } label: {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.uturn.backward")
                        .font(.system(size: 12, weight: .semibold))
                        .accessibilityHidden(true)
                    Text(L.readerBackToPage(page: page))
                        .font(.ui(13, weight: .medium))
                        .lineLimit(1)
                }
                .foregroundStyle(theme.silk)
                .padding(.horizontal, 12)
                .frame(minHeight: 30)
                .background(theme.selected, in: .capsule)
                .frame(minHeight: mac ? 30 : 44)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilitySortPriority(0.5)
            Spacer(minLength: 0)
        }
    }

    /// The series over the line, which opens Go to page.
    private var titleStack: some View {
        Button { model.askingPage = true } label: {
            VStack(spacing: 1) {
                if let title = model.view?.title, !title.isEmpty {
                    Text(title)
                        .font(.showTitle(size: 14, relativeTo: .subheadline)).fontWeight(.bold)
                        .foregroundStyle(theme.silk)
                        .lineLimit(big ? 2 : 1)
                }
                Text(Self.line(model: model, index: scrub ?? model.position.index))
                    .font(.ui(12)).monospacedDigit()
                    .foregroundStyle(theme.haze)
                    .lineLimit(big ? 3 : 1)
                    // Page 1 of 37 crossfades to credit page and back; turns stay as they are.
                    .contentTransition(.opacity)
                    .animation(paper ? nil : .fadeIn, value: creditKey)
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity, minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityHint(L.readerGoToPage)
        .accessibilitySortPriority(1)
        .popover(isPresented: $model.askingPage) { GoToPagePrompt(model: model, theme: theme) }
    }

    /// Changes when a credit page comes or goes, or is kept: the line crossfades then.
    private var creditKey: String {
        "\(model.onCredit.map(String.init) ?? "")|\(model.keeping?.credit.page.description ?? "")" // i18n-skip: an animation key
    }

    private var chaptersButton: some View {
        button("list.bullet", L.readerChapters) { model.sheet = .chapters }
            .accessibilitySortPriority(3)
            .popover(isPresented: sheetBinding(.chapters), attachmentAnchor: .point(.top)) {
                ReaderChaptersSheet(model: model, theme: theme)
            }
    }

    private var settingsButton: some View {
        Button { model.sheet = .settings } label: {
            Group {
                // Aa cannot shrink into the button at the largest sizes; the glyph can.
                if big {
                    Image(systemName: "textformat.size").font(.system(size: 18, weight: .medium))
                } else {
                    Text(L.readerTextLabel).font(.system(size: 17, weight: .semibold))
                }
            }
            .foregroundStyle(theme.silk)
            .frame(width: 44, height: 44)
            .background(model.sheet == .settings ? theme.selected : Color.clear, in: .circle)
            .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L.readerSettingsOpen)
        .accessibilitySortPriority(2)
        .popover(isPresented: sheetBinding(.settings), attachmentAnchor: .point(.top)) {
            ReaderSettingsSheet(model: model, theme: theme, mac: mac)
        }
    }

    /// The scrubber between the page shown and the count, read the way the book runs.
    private var scrubberRow: some View {
        let ends = Self.ends(model: model, index: scrub ?? model.position.index)
        return HStack(spacing: 10) {
            if let ends { end(rtl ? ends.total : ends.at) }
            ReaderScrubber(model: model, theme: theme, rtl: rtl, scrub: $scrub)
                .accessibilitySortPriority(0)
                .frame(height: 28)
            if let ends { end(rtl ? ends.at : ends.total) }
        }
    }

    /// A number at the scrubber's end. None while a credit page shows, which has no number of
    /// its own; a count that changes rolls to the new one.
    ///
    /// The roll is drawn in a drawing group. Without one, macOS draws each frame of the roll
    /// through Core Graphics in software, and every frame adds glyph bitmaps to a process-wide
    /// font cache. A page number rolls on every turn, so a long read would fill that cache with
    /// tens of megabytes that stay after the reader closes.
    private func end(_ number: Int?) -> some View {
        Text(verbatim: number.map { "\($0)" } ?? "")
            .font(.ui(12)).monospacedDigit()
            .foregroundStyle(theme.haze)
            .lineLimit(1)
            .fixedSize()
            .contentTransition(.numericText(value: Double(number ?? 0)))
            .drawingGroup()
            .animation(paper ? nil : .standard, value: number)
            .accessibilityHidden(true)
    }

    /// The numbers at the scrubber's ends: the page, image or novel page shown, and how many.
    /// A novel in one scroll has none.
    static func ends(model: ReaderModel, index: Int) -> (at: Int?, total: Int)? {
        if model.novel != nil {
            if model.choice(.mode) == "scrolled" { return nil }
            return (index + 1, max(model.position.count, 1))
        }
        if model.view?.mode == .strip { return (index + 1, max(model.position.count, 1)) }
        // A credit page only shown has no number, and the count stays; one kept counts at once.
        let onCredit = model.shownCredit != nil && index == model.position.index
        if onCredit, model.keeping == nil { return (nil, max(model.pageCount, 1)) }
        return (model.shownPageNumber(of: index), max(model.shownPageCount, 1))
    }

    private func button(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 17, weight: .medium)).foregroundStyle(theme.silk)
                .frame(width: 44, height: 44).contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }

    private func sheetBinding(_ sheet: ReaderModel.Sheet) -> Binding<Bool> {
        Binding(
            get: { model.sheet == sheet },
            set: { if !$0, model.sheet == sheet { model.sheet = nil } })
    }

    /// The group's line, "Chapter 12 · pages 8 and 9", for the chapter the person is in: the
    /// one a strip ran on from while it is still above the middle of the screen.
    static func line(model: ReaderModel, index: Int) -> String {
        if let past = model.inTrail {
            return L.readerGroupLineStrip(chapter: chapterName(past.label), image: index + 1, total: max(model.position.count, 1))
        }
        let chapter = chapterName(model.view?.chapter.label ?? "")
        let count = max(model.position.count, 1)
        if model.novel != nil {
            if model.choice(.mode) == "scrolled" {
                return L.readerGroupLineScroll(chapter: chapter, percent: Int((Double(index) / Double(max(count - 1, 1)) * 100).rounded()))
            }
            return L.readerGroupLineNovel(chapter: chapter, page: index + 1, total: count)
        }
        if model.view?.mode == .strip { return L.readerGroupLineStrip(chapter: chapter, image: index + 1, total: count) }
        // A credit page shows in place of the number until it is kept: "Chapter 87 · credit page".
        if index == model.position.index, let place = model.creditPlace {
            if model.keeping?.credit.page == model.onCredit {
                return L.readerGroupLine(chapter: chapter, page: model.shownPageNumber(of: index), total: max(model.shownPageCount, 1))
            }
            let words = place.of > 1 ? L.readerCreditPageOf(n: place.n, total: place.of) : L.readerCreditPage
            return L.readerGroupLinePlace(chapter: chapter, place: words)
        }
        if model.isEndPage(index) { return L.readerGroupLineEnd(chapter: chapter) }
        if model.leaves.indices.contains(index), model.leaves[index].parts.count > 1 {
            let pages = model.leaves[index].parts.map { Int($0.page) + 1 }.sorted()
            return L.readerGroupLineSpread(chapter: chapter, first: pages[0], second: pages[1])
        }
        return L.readerGroupLine(chapter: chapter, page: model.shownPageNumber(of: index), total: max(model.shownPageCount, 1))
    }

    /// A chapter named in the reader: the source's own words, or "Chapter 12" when they are only
    /// its number.
    static func chapterName(_ label: String) -> String {
        let words = label.trimmingCharacters(in: .whitespaces)
        let bare = words.first?.isASCII == true && words.first?.isNumber == true
            && words.allSatisfy { $0.isASCII && ($0.isNumber || $0 == ".") }
        return bare ? L.readerChapterLine(chapter: words) : label
    }
}

/// Go to page: the field and Go, which the number pad has no key for. A view of its own, so the
/// field's focus lives in the popover that shows it.
private struct GoToPagePrompt: View {
    let model: ReaderModel
    let theme: ReaderTheme
    @State private var text = ""
    @FocusState private var focused: Bool

    private var total: Int { model.goToPageTotal }

    /// The number typed, when it is a page there is.
    private var target: Int? { model.goToPageTarget(text) }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L.readerGoToPage).font(.ui(15, weight: .semibold)).foregroundStyle(theme.silk)
            HStack(spacing: 8) {
                #if os(iOS)
                GoToPageField(text: $text, placeholder: L.readerGoToPageField(total: total), submit: submit) {
                    model.askingPage = false
                }
                .frame(width: 140)
                #else
                TextField(L.readerGoToPageField(total: total), text: $text)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 140)
                    .focused($focused)
                    .onSubmit(submit)
                #endif
                Button(L.readerGoToPageSubmit, action: submit)
                    .buttonStyle(ReaderAmberStyle(theme: theme, height: 36).fixed())
                    .disabled(target == nil)
                    .opacity(target == nil ? 0.45 : 1)
            }
        }
        .padding(16)
        .presentationCompactAdaptation(.popover)
        #if !os(iOS)
        // The field takes the keys as the prompt shows. On iPad the field takes them itself.
        .defaultFocus($focused, true)
        .onAppear { focused = true }
        // Esc closes the prompt from the field, which holds the keys while it is up.
        .onKeyPress(.escape) {
            model.askingPage = false
            return .handled
        }
        #endif
    }

    private func submit() {
        model.goToPage(typed: text)
    }
}

/// The scrubber: a 3 pt track, a 14 pt thumb and an 11 pt ring where the person was before a
/// jump. In right-to-left it runs from the right.
struct ReaderScrubber: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let rtl: Bool
    @Binding var scrub: Int?
    /// The part tick the thumb last snapped to, for the light tap the snap gives.
    @State private var snapped: Int?

    var body: some View {
        GeometryReader { proxy in
            let count = max(model.position.count, 1)
            let width = proxy.size.width
            let index = scrub ?? model.position.index
            let fraction = count > 1 ? CGFloat(index) / CGFloat(count - 1) : 0
            let x = (rtl ? 1 - fraction : fraction) * width
            ZStack(alignment: .leading) {
                Capsule().fill(theme.track).frame(height: 3)
                Capsule().fill(theme.silk)
                    .frame(width: rtl ? width - x : x, height: 3)
                    .frame(maxWidth: .infinity, alignment: rtl ? .trailing : .leading)
                // Where each later part of the chapter starts; a part no group has is fainter.
                if count > 1 {
                    ForEach(model.partTicks) { tick in
                        let tf = CGFloat(tick.leaf) / CGFloat(count - 1)
                        Capsule().fill(theme.haze).frame(width: 2, height: 9)
                            .opacity(tick.missing ? 0.4 : 1)
                            .position(x: (rtl ? 1 - tf : tf) * width, y: proxy.size.height / 2)
                            .accessibilityHidden(true)
                    }
                }
                if let ring = model.ring, model.novel == nil, count > 1 {
                    let rf = CGFloat(ring) / CGFloat(count - 1)
                    Circle().stroke(theme.silk, lineWidth: 1.5).frame(width: 11, height: 11)
                        .position(x: (rtl ? 1 - rf : rf) * width, y: proxy.size.height / 2)
                }
                Circle().fill(theme.silk).frame(width: 14, height: 14)
                    .shadow(color: theme.shadow, radius: 3, y: 1)
                    .position(x: x, y: proxy.size.height / 2)
                if scrub != nil { preview(index: index).position(x: min(max(x, 48), width - 48), y: -66) }
            }
            .frame(maxHeight: .infinity)
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        let f = min(max(value.location.x / max(width, 1), 0), 1)
                        let at = Int(((rtl ? 1 - f : f) * CGFloat(count - 1)).rounded())
                        // The thumb snaps lightly onto a part's tick within a few points of it.
                        let near = model.partTicks.first { tick in
                            !tick.missing && abs(CGFloat(tick.leaf - at)) * width / CGFloat(max(count - 1, 1)) < 8
                        }
                        scrub = near?.leaf ?? at
                        snapped = near?.leaf
                    }
                    .onEnded { _ in
                        if let scrub { model.requestJump(scrub) }
                        scrub = nil
                        snapped = nil
                    })
            #if os(iOS)
            .sensoryFeedback(.selection, trigger: snapped) { _, new in new != nil }
            #endif
            .accessibilityElement()
            .accessibilityLabel(L.readerScrubber)
            .accessibilityValue(spoken(index: index, count: count))
            .accessibilityAdjustableAction { direction in
                model.requestJump(min(max(model.position.index + (direction == .increment ? 1 : -1), 0), count - 1))
            }
        }
    }

    /// "Page 8 of 38": pages as the chapter counts them, not leaves, so a spread is its first page.
    private func spoken(index: Int, count: Int) -> String {
        if model.novel == nil, model.view?.mode != .strip {
            let page = model.pageNumber(of: index)
            if let part = model.partStarting(atPage: page) { return L.readerPartStarts(part: Int(part), page: page) }
            if index == model.position.index, model.shownCredit != nil {
                return ReaderChrome.line(model: model, index: index)
            }
            return L.readerScrubberValue(page: model.shownPageNumber(of: index), total: max(model.shownPageCount, 1))
        }
        return L.readerScrubberValue(page: index + 1, total: count)
    }

    /// The preview's number: "Page 25 · part 2" on the first page of a later part, and "Page 14 ·
    /// part 4 · part 3 missing" when the part before it is missing.
    private func previewLabel(index: Int) -> String {
        guard model.novel == nil, model.view?.mode != .strip else { return "\(index + 1)" }
        let page = model.pageNumber(of: index)
        if let part = model.partStarting(atPage: page) {
            if let missing = model.partMissing(beforePage: page) {
                return L.readerPartPreviewMissing(page: page, part: Int(part), missing: Int(missing))
            }
            return L.readerPartPreview(page: page, part: Int(part))
        }
        return "\(page)"
    }

    /// A 96 pt thumbnail of the page under the thumb, with its number.
    @ViewBuilder private func preview(index: Int) -> some View {
        VStack(spacing: 4) {
            if model.novel == nil, model.leaves.indices.contains(index), let part = model.leaves[index].parts.first,
                let chapter = model.view?.chapter.id, let file = model.file(of: part, chapter: chapter)
            {
                PageImage(file: file, longest: 160)
                    .frame(width: 96, height: 136)
                    .clipShape(.rect(cornerRadius: 8))
            }
            Text(previewLabel(index: index))
                .font(.ui(13, weight: .semibold)).monospacedDigit().foregroundStyle(theme.silk)
                .lineLimit(1).fixedSize()
                .padding(.horizontal, 10).padding(.vertical, 3)
                .background(theme.glassSolid, in: .capsule)
        }
        .frame(width: 96)
        .offset(y: model.novel == nil ? -40 : 0)
        .task(id: index) {
            // The page under the thumb may not have been asked for yet.
            guard model.novel == nil, model.leaves.indices.contains(index), let part = model.leaves[index].parts.first,
                let chapter = model.view?.chapter.id, model.file(of: part, chapter: chapter) == nil
            else { return }
            model.needPage(part.page)
        }
    }
}

/// The notice capsule.
struct ReaderNotices: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    /// The chrome group's height, which a novel's notice clears while it shows.
    let groupHeight: CGFloat
    @Environment(\.readerPaper) private var paper
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var type
    @AccessibilityFocusState private var noticeFocused: Bool
    @State private var hovering = false
    @Environment(\.undoManager) private var undoManager

    private var wide: Bool { mac || sizeClass == .regular }
    /// A novel's notices sit in its bottom margin, in place of the pages-left line, clear of the
    /// text.
    private var inMargin: Bool { model.novel != nil }

    /// Where a novel's notice sits above the bottom: in the margin, or over the chrome group
    /// while it shows, since the group sits in the same margin.
    private var marginBottom: CGFloat {
        guard inMargin, model.chromeShown else { return mac ? 44 : 16 }
        return ReaderChrome.bottom(mac: mac, wide: wide) + groupHeight + 12
    }

    var body: some View {
        ZStack {
            VStack {
                if !inMargin { notices }
                Spacer()
                if inMargin { notices }
            }
            .padding(.top, 62)
            .padding(.bottom, marginBottom)
            .animation(paper || reduceMotion ? nil : .standard, value: marginBottom)
            if creditLineShows {
                // The pages skipped before page 1, named under the top where the demo's credit
                // sits: centred on iPhone and iPad, top trailing on the Mac.
                VStack {
                    HStack {
                        if mac { Spacer() }
                        creditLine
                    }
                    Spacer()
                }
                .padding(.top, mac ? 16 : (wide ? 30 : 62))
                .padding(.trailing, mac ? 16 : 0)
                .transition(.opacity)
            }
            if model.notice == nil, !creditLineShows, let credit = model.view?.demoCredit, model.position.index == 0 {
                // The demo's credit on page 1, where its licence asks for it: centred under the
                // top on iPhone and iPad, at the top's trailing end on the Mac, as the board has it.
                // It waits while the credit line holds the same spot, and comes back after it.
                VStack {
                    HStack {
                        if mac { Spacer() }
                        self.credit(L.readerDemoCredit(title: credit.series, author: credit.author, licence: credit.licence ?? ""))
                    }
                    Spacer()
                }
                .padding(.top, mac ? 10 : 62)
                .padding(.trailing, mac ? 12 : 0)
                .transition(.opacity)
            }
        }
        .animation(paper ? nil : (model.notice == nil ? .fadeOut : .fadeIn), value: model.notice?.id)
        .animation(paper ? nil : .fadeOut, value: model.position.index == 0)
        .animation(paper ? nil : (creditLineShows ? .fadeIn : .fadeOut), value: creditLineShows)
        // Every notice is said, and its time stops while VoiceOver or the pointer rests on it.
        .onChange(of: model.notice?.id) { _, _ in announce(model.notice) }
        // The line is said once when page 1 settles, politely, so it never cuts off the page.
        .onChange(of: model.creditLine) { _, up in
            guard up else { return }
            var words = AttributedString(L.readerCreditPagesSkipped(count: model.startSkipped.count))
            words.accessibilitySpeechAnnouncementPriority = .low
            AccessibilityNotification.Announcement(words).post()
        }
        // ⌘Z undoes Not a credit page while its Undo lasts, as the notice's button does.
        .onChange(of: model.keeping) { _, keeping in
            undoManager?.removeAllActions(withTarget: model)
            guard keeping != nil else { return }
            undoManager?.registerUndo(withTarget: model) { model in
                MainActor.assumeIsolated { model.undoKeep() }
            }
            undoManager?.setActionName(L.readerCreditNot)
        }
        .onChange(of: noticeFocused || hovering) { _, held in model.noticeHeld = held }
    }

    @ViewBuilder private var notices: some View {
        if let notice = model.notice {
            capsule(notice)
                .accessibilityFocused($noticeFocused)
                .onHover { hovering = $0 }
                // A credit page's notices crossfade into each other: Not a credit page, then Undo.
                .id(notice.local == nil ? 0 : notice.id)
                .transition(.opacity)
        }
    }

    private func announce(_ line: ReaderNoticeLine?) {
        guard let line else { return }
        var words = line.text
        if let label = Self.label(line) { words += ". " + label } // i18n-skip: a pause in speech
        AccessibilityNotification.Announcement(words).post()
    }

    /// The notice's button, the engine's action or the app's own.
    static func label(_ line: ReaderNoticeLine) -> String? {
        if let local = line.local { return label(local) }
        return line.action.flatMap { label($0) }
    }

    static func label(_ action: ReaderLocalAction) -> String {
        switch action {
        case .keepCredit: L.readerCreditNot
        case .undoKeep: L.actionUndo
        }
    }

    /// Page 1's line: on page 1 while it holds, and again with the chrome there. A notice takes
    /// its place while one shows.
    private var creditLineShows: Bool {
        guard model.novel == nil, model.view?.mode != .strip, model.notice == nil, model.onCredit == nil,
              model.current == 0, model.position.index == 0, !model.startSkipped.isEmpty else { return false }
        return model.creditLine || model.chromeShown
    }

    /// "1 credit page skipped": the demo credit's capsule, 32 high and 13 point, and the whole
    /// capsule is the button. It grows to accessibility 1 and stops, so it never covers the page.
    private var creditLine: some View {
        // Page 1 gives way to the credit page by the same crossfade as a turn.
        // The credit page only appears in place, so it fades in.
        Button { withAnimation(paper ? nil : .fadeIn) { model.showStartCredits() } } label: {
            Text(L.readerCreditPagesSkipped(count: model.startSkipped.count))
                .font(.ui(13, weight: .medium))
                .foregroundStyle(theme.silk)
                .lineLimit(1)
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
                .frame(minHeight: 32)
                .background(theme.glassSolid, in: .capsule)
                .glassEffect(.regular, in: .capsule)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .dynamicTypeSize(...DynamicTypeSize.accessibility1)
        .pointerHover()
        .padding(.horizontal, mac ? 0 : 16)
        .accessibilityHint(L.readerCreditShowHint)
    }

    private func credit(_ text: String) -> some View {
        let height: CGFloat = mac ? 28 : 32
        return HStack(spacing: 8) {
            Image(systemName: "info.circle").font(.ui(mac ? 12 : 13)).accessibilityHidden(true)
            Text(text).font(.ui(mac ? 12 : 13)).lineLimit(1)
        }
        .foregroundStyle(theme.silk)
        .padding(.horizontal, height / 2)
        .frame(height: height)
        .background(theme.glassSolid, in: .capsule)
        .glassEffect(.regular, in: .capsule)
        .padding(.horizontal, mac ? 0 : 16)
        .accessibilityElement(children: .combine)
    }

    private func capsule(_ notice: ReaderNoticeLine) -> some View {
        let small = inMargin
        let text = Text(notice.text).font(.ui(small ? 13 : 15)).foregroundStyle(theme.silk).lineLimit(2)
        let action = Self.label(notice).map { label in
            Button(label) {
                if let local = notice.local { model.act(local) } else if let action = notice.action { model.act(action) }
            }
            .font(.ui(small ? 13 : 15, weight: .semibold)).foregroundStyle(theme.silk).buttonStyle(.plain)
            .underline()
            .frame(minHeight: 44)
            .pointerHover()
        }
        // The words and their button side by side, the words wrapping onto a second line when
        // long; at the accessibility sizes the button goes under them. The choice comes from
        // the text size, not a measure, so a scroll under the notice never flips it mid-layout.
        return Group {
            if type.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 0) {
                    text.fixedSize(horizontal: false, vertical: true)
                        .padding(.top, 10).padding(.bottom, action == nil ? 10 : 0)
                    action
                }
            } else {
                HStack(spacing: 12) {
                    text.fixedSize(horizontal: false, vertical: true).layoutPriority(1)
                    action?.fixedSize()
                }
                .padding(.vertical, 4)
            }
        }
        // The capsule stays a notice at the largest sizes, not a sheet over the page.
        .dynamicTypeSize(...DynamicTypeSize.accessibility1)
        .padding(.horizontal, 18)
        .frame(minHeight: small ? 32 : 44)
        // The board's blur hides the text behind the notice. Where the glass draws flat, as in
        // the dev harness, the near-solid tint keeps the words from showing through.
        .background(theme.glassSolid, in: .rect(cornerRadius: small ? 16 : 22))
        .glassEffect(.regular, in: .rect(cornerRadius: small ? 16 : 22))
        .padding(.horizontal, 16)
        .frame(maxWidth: 560)
        .accessibilityElement(children: .combine)
    }

    static func label(_ action: ReaderAction) -> String? {
        switch action {
        case .startOver: L.readerStartOver
        case .goBack: L.readerGoBack
        case .changeMode: L.readerChange
        }
    }
}
