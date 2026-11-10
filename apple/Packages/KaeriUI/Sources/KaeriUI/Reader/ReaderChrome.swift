// The reader's chrome: the group at the bottom with the scrubber, the notice capsule and the
// Back to page pill. All of it is glass over the page.

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
    /// The group's height, for the Back to page pill to clear it.
    @Binding var groupHeight: CGFloat
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.dynamicTypeSize) private var type
    @State private var scrub: Int?
    @State private var goToText = ""
    @FocusState private var fieldFocused: Bool
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
            }
            .multilineTextAlignment(.center)
            .frame(maxWidth: .infinity, minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityHint(L.readerGoToPage)
        .accessibilitySortPriority(1)
        .popover(isPresented: $model.askingPage) { goTo }
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

    private func end(_ number: Int) -> some View {
        Text(verbatim: "\(number)")
            .font(.ui(12)).monospacedDigit()
            .foregroundStyle(theme.haze)
            .lineLimit(1)
            .fixedSize()
            .accessibilityHidden(true)
    }

    /// The numbers at the scrubber's ends: the page, image or novel page shown, and how many.
    /// A novel in one scroll has none.
    static func ends(model: ReaderModel, index: Int) -> (at: Int, total: Int)? {
        if model.novel != nil {
            if model.choice(.mode) == "scrolled" { return nil }
            return (index + 1, max(model.position.count, 1))
        }
        if model.view?.mode == .strip { return (index + 1, max(model.position.count, 1)) }
        return (model.pageNumber(of: index), max(model.pageCount, 1))
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
            return L.readerGroupLineStrip(chapter: past.label, image: index + 1, total: max(model.position.count, 1))
        }
        let chapter = model.view?.chapter.label ?? ""
        let count = max(model.position.count, 1)
        if model.novel != nil {
            if model.choice(.mode) == "scrolled" {
                return L.readerGroupLineScroll(chapter: chapter, percent: Int((Double(index) / Double(max(count - 1, 1)) * 100).rounded()))
            }
            return L.readerGroupLineNovel(chapter: chapter, page: index + 1, total: count)
        }
        if model.view?.mode == .strip { return L.readerGroupLineStrip(chapter: chapter, image: index + 1, total: count) }
        if model.isEndPage(index) { return L.readerGroupLineEnd(chapter: chapter) }
        if model.leaves.indices.contains(index), model.leaves[index].parts.count > 1 {
            let pages = model.leaves[index].parts.map { Int($0.page) + 1 }.sorted()
            return L.readerGroupLineSpread(chapter: chapter, first: pages[0], second: pages[1])
        }
        return L.readerGroupLine(chapter: chapter, page: model.pageNumber(of: index), total: max(model.pageCount, 1))
    }

    private var goToTotal: Int { model.novel != nil || model.view?.mode == .strip ? max(model.position.count, 1) : max(model.pageCount, 1) }

    /// The number typed, when it is a page there is.
    private var goToTarget: Int? {
        guard let number = Int(goToText.trimmingCharacters(in: .whitespaces)), (1...goToTotal).contains(number) else { return nil }
        return number
    }

    /// Go to page: the field and Go, which the number pad has no key for.
    private var goTo: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L.readerGoToPage).font(.ui(15, weight: .semibold)).foregroundStyle(theme.silk)
            HStack(spacing: 8) {
                TextField(L.readerGoToPageField(total: goToTotal), text: $goToText)
                    .textFieldStyle(.roundedBorder)
                    #if os(iOS)
                    .keyboardType(.numberPad)
                    #endif
                    .frame(width: 140)
                    .focused($fieldFocused)
                    .onSubmit(submit)
                Button(L.readerGoToPageSubmit, action: submit)
                    .buttonStyle(ReaderAmberStyle(theme: theme, height: 36).fixed())
                    .disabled(goToTarget == nil)
                    .opacity(goToTarget == nil ? 0.45 : 1)
            }
        }
        .padding(16)
        .presentationCompactAdaptation(.popover)
        .onAppear { fieldFocused = true }
    }

    private func submit() {
        guard let page = goToTarget else { return }
        model.askingPage = false
        goToText = ""
        model.requestJump(model.novel != nil || model.view?.mode == .strip ? page - 1 : model.leaf(ofPage: page))
    }
}

/// The scrubber: a 3 pt track, a 14 pt thumb and an 11 pt ring where the person was before a
/// jump. In right-to-left it runs from the right.
struct ReaderScrubber: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let rtl: Bool
    @Binding var scrub: Int?

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
                        scrub = Int(((rtl ? 1 - f : f) * CGFloat(count - 1)).rounded())
                    }
                    .onEnded { _ in
                        if let scrub { model.requestJump(scrub) }
                        scrub = nil
                    })
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
            return L.readerScrubberValue(page: model.pageNumber(of: index), total: max(model.pageCount, 1))
        }
        return L.readerScrubberValue(page: index + 1, total: count)
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
            Text("\(model.novel == nil && model.view?.mode != .strip ? model.pageNumber(of: index) : index + 1)")
                .font(.ui(13, weight: .semibold)).monospacedDigit().foregroundStyle(theme.silk)
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

/// The notice capsule, and the Back to page pill.
struct ReaderNotices: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    let size: CGSize
    /// The chrome group's height, which the pill clears while it shows.
    let groupHeight: CGFloat
    @Environment(\.readerPaper) private var paper
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var type
    @AccessibilityFocusState private var noticeFocused: Bool
    @AccessibilityFocusState private var pillFocused: Bool
    @State private var hovering = false

    private var wide: Bool { mac || sizeClass == .regular }
    /// A novel's notices sit in its bottom margin, in place of the pages-left line, clear of the
    /// text.
    private var inMargin: Bool { model.novel != nil }

    /// Where the pill sits above the bottom: over the chrome group while it shows on a phone,
    /// where the group spans the width.
    private var pillBottom: CGFloat {
        guard model.chromeShown, !wide else { return 40 }
        return ReaderChrome.bottom(mac: mac, wide: wide) + groupHeight + 12
    }

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
            if model.notice == nil, let credit = model.view?.demoCredit, model.position.index == 0 {
                // The demo's credit on page 1, where its licence asks for it: centred under the
                // top on iPhone and iPad, at the top's trailing end on the Mac, as the board has it.
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
            VStack {
                Spacer()
                HStack {
                    if let back = model.backTo { pill(back).transition(.opacity) }
                    Spacer()
                }
                .padding(.leading, 16).padding(.bottom, pillBottom)
            }
            .animation(paper || reduceMotion ? nil : .standard, value: pillBottom)
        }
        .animation(paper ? nil : (model.notice == nil ? .fadeOut : .fadeIn), value: model.notice?.id)
        .animation(paper ? nil : .fadeOut, value: model.position.index == 0)
        .animation(paper ? nil : (model.backTo == nil ? .fadeOut : .fadeIn), value: model.backTo?.id)
        // Every notice is said, and its time stops while VoiceOver or the pointer rests on it.
        .onChange(of: model.notice?.id) { _, _ in announce(model.notice) }
        .onChange(of: model.backTo?.id) { _, _ in announce(model.backTo) }
        .onChange(of: noticeFocused || pillFocused || hovering) { _, held in model.noticeHeld = held }
    }

    @ViewBuilder private var notices: some View {
        if let notice = model.notice {
            capsule(notice)
                .accessibilityFocused($noticeFocused)
                .onHover { hovering = $0 }
                .transition(.opacity)
        }
    }

    private func announce(_ line: ReaderNoticeLine?) {
        guard let line else { return }
        var words = line.text
        if let action = line.action, let label = Self.label(action) { words += ". " + label } // i18n-skip: a pause in speech
        AccessibilityNotification.Announcement(words).post()
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
        let action = notice.action.flatMap { action in
            Self.label(action).map { label in
                Button(label) { model.act(action) }
                    .font(.ui(small ? 13 : 15, weight: .semibold)).foregroundStyle(theme.silk).buttonStyle(.plain)
                    .underline()
                    .frame(minHeight: 44)
            }
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

    private func pill(_ line: ReaderNoticeLine) -> some View {
        Button { model.act(.backTo) } label: {
            HStack(spacing: 8) {
                Image(systemName: "arrow.uturn.backward").font(.system(size: 13, weight: .semibold))
                Text(line.text).font(.ui(15, weight: .medium))
            }
            .foregroundStyle(theme.silk)
            .padding(.horizontal, 16)
            .frame(height: 40)
            .background(theme.glass, in: .capsule)
            .glassEffect(.regular, in: .capsule)
        }
        .buttonStyle(.plain)
        .accessibilityFocused($pillFocused)
        .onHover { hovering = $0 }
    }

    static func label(_ action: ReaderAction) -> String? {
        switch action {
        case .startOver: L.readerStartOver
        case .goBack: L.readerGoBack
        case .changeMode: L.readerChange
        case .backTo: nil
        }
    }
}
