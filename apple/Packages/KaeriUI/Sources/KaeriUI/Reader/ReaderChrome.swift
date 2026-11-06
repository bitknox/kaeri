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
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var scrub: Int?
    @State private var goToText = ""

    private var wide: Bool { mac || sizeClass == .regular }
    private var rtl: Bool { model.view?.direction == .rightToLeft && model.view?.mode != .strip && model.medium == .manga }

    var body: some View {
        VStack {
            Spacer()
            group
                .frame(maxWidth: wide ? 640 : .infinity)
                .padding(.horizontal, wide ? 0 : 12)
                .padding(.bottom, mac ? 20 : (wide ? 24 : 28))
        }
        .opacity(model.chromeShown ? 1 : 0)
        .allowsHitTesting(model.chromeShown)
        .accessibilityHidden(!model.chromeShown)
    }

    private var group: some View {
        VStack(spacing: 0) {
            ReaderScrubber(model: model, theme: theme, rtl: rtl, scrub: $scrub)
                .frame(height: 28)
                .padding(.horizontal, 18)
                .padding(.top, 6)
            HStack(spacing: 4) {
                button("xmark", L.readerClose, action: onClose)
                Button { model.askingPage = true } label: {
                    Text(line).font(.ui(15, weight: .medium)).foregroundStyle(theme.silk)
                        .monospacedDigit().lineLimit(1).minimumScaleFactor(0.8)
                        .frame(maxWidth: .infinity, minHeight: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.readerGoToPage)
                .popover(isPresented: $model.askingPage) { goTo }
                button("list.bullet", L.readerChapters) { model.sheet = .chapters }
                    .popover(isPresented: sheetBinding(.chapters), attachmentAnchor: .point(.top)) {
                        ReaderChaptersSheet(model: model, theme: theme)
                    }
                button("slider.horizontal.3", L.readerSettingsOpen) { model.sheet = .settings }
                    .popover(isPresented: sheetBinding(.settings), attachmentAnchor: .point(.top)) {
                        ReaderSettingsSheet(model: model, theme: theme, mac: mac)
                    }
            }
            .padding(.horizontal, 6)
            .frame(height: 44)
            .padding(.bottom, 6)
        }
        .glassEffect(.regular, in: .rect(cornerRadius: 30))
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

    private var line: String {
        let chapter = model.view?.chapter.label ?? ""
        let index = scrub ?? model.position.index
        let count = max(model.position.count, 1)
        if model.novel != nil {
            if model.choice(.mode) == "scrolled" {
                return L.readerGroupLineScroll(chapter: chapter, percent: Int((Double(index) / Double(max(count - 1, 1)) * 100).rounded()))
            }
            return L.readerGroupLineNovel(chapter: chapter, page: index + 1, total: count)
        }
        if model.view?.mode == .strip { return L.readerGroupLineStrip(chapter: chapter, image: index + 1, total: count) }
        if model.leaves.indices.contains(index), model.leaves[index].parts.count > 1 {
            let pages = model.leaves[index].parts.map { Int($0.page) + 1 }.sorted()
            return L.readerGroupLineSpread(chapter: chapter, first: pages[0], second: pages[1])
        }
        return L.readerGroupLine(chapter: chapter, page: model.pageNumber(of: index), total: max(model.pageCount, 1))
    }

    private var goTo: some View {
        let total = model.novel != nil ? model.position.count : max(model.pageCount, 1)
        return VStack(alignment: .leading, spacing: 10) {
            Text(L.readerGoToPage).font(.ui(15, weight: .semibold))
            TextField(L.readerGoToPageField(total: total), text: $goToText)
                .textFieldStyle(.roundedBorder)
                #if os(iOS)
                .keyboardType(.numberPad)
                #endif
                .frame(width: 180)
                .onSubmit(submit)
        }
        .padding(16)
        .presentationCompactAdaptation(.popover)
    }

    private func submit() {
        model.askingPage = false
        guard let number = Int(goToText.trimmingCharacters(in: .whitespaces)) else { return }
        goToText = ""
        let total = model.novel != nil ? model.position.count : max(model.pageCount, 1)
        let page = min(max(number, 1), total)
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
            .accessibilityValue("\(index + 1) / \(count)")
            .accessibilityAdjustableAction { direction in
                model.requestJump(min(max(model.position.index + (direction == .increment ? 1 : -1), 0), count - 1))
            }
        }
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
    }
}

/// The notice capsule, and the Back to page pill.
struct ReaderNotices: View {
    let model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            VStack {
                if let notice = model.notice { capsule(notice).transition(.opacity.combined(with: .offset(y: -8))) }
                Spacer()
            }
            .padding(.top, 62)
            VStack {
                Spacer()
                HStack {
                    if let back = model.backTo { pill(back).transition(.opacity) }
                    Spacer()
                }
                .padding(.leading, 16).padding(.bottom, 40)
            }
        }
        .animation(reduceMotion ? nil : .standard, value: model.notice?.id)
        .animation(reduceMotion ? nil : .standard, value: model.backTo?.id)
    }

    private func capsule(_ notice: ReaderNoticeLine) -> some View {
        HStack(spacing: 12) {
            Text(notice.text).font(.ui(15)).foregroundStyle(theme.silk).lineLimit(2)
            if let action = notice.action, let label = Self.label(action) {
                Button(label) { model.act(action) }
                    .font(.ui(15, weight: .semibold)).foregroundStyle(theme.silk).buttonStyle(.plain)
                    .underline()
            }
        }
        .padding(.horizontal, 18)
        .frame(minHeight: 44)
        .glassEffect(.regular, in: .capsule)
        .padding(.horizontal, 16)
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
            .glassEffect(.regular, in: .capsule)
        }
        .buttonStyle(.plain)
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
