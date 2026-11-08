// The reader's sheets: settings and the chapters and pages list. They open as popovers on iPad
// and Mac and as sheets on iPhone, and the page behind them stays undimmed.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

// MARK: Settings

struct ReaderSettingsSheet: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    let mac: Bool
    @State private var more = false
    @State private var query = ""
    @AppStorage(ReaderDefaults.background) private var background = "theme"
    @AppStorage(ReaderDefaults.gap) private var gap = "none"
    @AppStorage(ReaderDefaults.step) private var step = "panel"
    @AppStorage(ReaderDefaults.system) private var system = false
    @Environment(\.dynamicTypeSize) private var type
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// The reading mode picker, pushed inside the sheet.
    @State private var pickingMode = false
    @State private var contentHeight: CGFloat = 560

    private var novel: Bool { model.medium == .novel }
    private var strip: Bool { model.view?.mode == .strip }
    private var popover: Bool { mac || ReaderDevice.isPad }
    /// At the accessibility sizes a row of chips becomes a list with a check.
    private var big: Bool { type.isAccessibilitySize }

    var body: some View {
        ZStack(alignment: .top) {
            if pickingMode {
                modePicker
                    .transition(reduceMotion ? .opacity : .push(from: .trailing))
            } else {
                main
                    .transition(reduceMotion ? .opacity : .push(from: .leading))
            }
        }
        // A popover is as tall as what it holds, up to 560 pt, and scrolls past that.
        .frame(width: popover ? 380 : nil, height: popover ? min(contentHeight, 560) : nil, alignment: .top)
        .clipped()
        .presentationDetents([.fraction(0.55), .large])
        .presentationBackgroundInteraction(.enabled)
        .presentationBackground(theme.glassSolid)
        .presentationCompactAdaptation(.sheet)
        .preferredColorScheme(system ? nil : theme.scheme)
        .foregroundStyle(theme.silk)
    }

    private var main: some View {
        scrolling {
            VStack(alignment: .leading, spacing: 22) {
                ForEach(groups, id: \.scope.hashValue) { group in
                    VStack(alignment: .leading, spacing: 14) {
                        header(group.title)
                        ForEach(group.items) { $0.view }
                    }
                }
                paperSection
                moreSection
            }
        }
    }

    /// The sheet's scroll, which reports its content's height for the popover to size to.
    private func scrolling(@ViewBuilder _ content: () -> some View) -> some View {
        ScrollView {
            content()
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
        }
        .scrollIndicators(.automatic)
        .scrollIndicatorsFlash(onAppear: true)
    }

    // MARK: Groups by scope

    private struct Item: Identifiable {
        let id: String
        let view: AnyView
    }

    private struct ScopeGroup {
        let scope: SettingScope
        let title: String
        let items: [Item]
    }

    private func item(_ id: String, _ view: some View) -> Item { Item(id: id, view: AnyView(view)) }

    /// The rows, each under the heading of where its change applies: this series, every title of
    /// the medium, or this device.
    private var groups: [ScopeGroup] {
        var rows: [(SettingScope, Item)] = []
        func add(_ key: ReaderSettingKey, _ view: some View) {
            guard model.row(key) != nil else { return }
            rows.append((model.row(key)?.scope ?? .device, item("\(key)", view)))
        }
        rows.append((.series, item("mode", modeSection)))
        add(.look, lookRow)
        if novel {
            add(.font, segments(L.readerFont, [(L.readerFontSerif, "serif"), (L.readerFontSans, "sans")], .font))
            add(.textSize, stepper(L.readerTextSize, .textSize, 0.1, 0.5...3.0))
            add(.lineSpacing, amounts(L.readerLineSpacing, .lineSpacing, [(L.readerLineTight, 1.2), (L.readerLineNormal, 1.55), (L.readerLineLoose, 1.8)]))
            add(.margins, amounts(L.readerMargins, .margins, [(L.readerMarginsNarrow, 0.6), (L.readerMarginsNormal, 1.0), (L.readerMarginsWide, 1.6)]))
            add(.justify, toggle(L.readerJustify, .justify))
        } else {
            if strip {
                // Fit and Crop borders mean nothing to a strip; its width does, where it is not
                // the screen's.
                if popover, model.amount(.stripWidth) != nil {
                    rows.append((.device, item("stripWidth", stepper(L.readerStripWidth, .stripWidth, 0.1, 0.3...1.0))))
                }
            } else {
                add(.fit, segments(L.readerFit, [(L.readerFitScreen, "screen"), (L.readerFitWidth, "width")], .fit))
                add(.cropBorders, toggle(L.readerCropBorders, .cropBorders))
            }
            rows.append((.device, item("background", chips(
                L.readerBackground, [(L.readerBackgroundTheme, "theme"), (L.readerBackgroundBlack, "black"), (L.readerBackgroundWhite, "white")],
                background) { background = $0 })))
        }
        let medium = novel ? L.readerAllNovels : (strip ? L.readerAllStrips : L.readerAllManga)
        let device = novel ? medium : (strip ? L.readerAllStripsHere : L.readerAllMangaHere)
        var out: [ScopeGroup] = []
        for scope in [SettingScope.series, .medium, .device] {
            let items = rows.filter { row in
                switch (scope, row.0) {
                case (.series, .series), (.series, .seriesHere), (.medium, .medium), (.device, .device): true
                default: false
                }
            }.map(\.1)
            guard !items.isEmpty else { continue }
            let title = scope == .series ? L.readerThisSeries : (scope == .medium ? medium : device)
            // A novel has no separate device heading: its device rows join the medium's.
            if novel, scope == .device, let at = out.firstIndex(where: { $0.scope == .medium }) {
                out[at] = ScopeGroup(scope: .medium, title: medium, items: out[at].items + items)
            } else {
                out.append(ScopeGroup(scope: scope, title: title, items: items))
            }
        }
        return out
    }

    private var lookRow: some View {
        let options = [
            (L.readerLookSystem, "system"), (L.readerLookPaper, "paper"), (L.readerLookLight, "light"),
            (L.readerLookSepia, "sepia"), (L.readerLookDark, "dark"),
        ]
        return chips(L.readerLook, options, system ? "system" : (model.choice(.look) ?? "dark")) { name in
            if name == "system" {
                system = true
            } else {
                system = false
                model.set(.look, .choice(name: name))
            }
        }
    }

    // MARK: Sections

    private var modeSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            if novel {
                segments(L.readerReadingMode, [(L.readerModePaged, "paged"), (L.readerModeScrolled, "scrolled")], .mode)
            } else {
                // The board's row: the mode in use, which pushes the picker.
                Button {
                    withAnimation(.standard) { pickingMode = true }
                } label: {
                    // At the accessibility sizes the value goes under the label.
                    let row = big ? AnyLayout(VStackLayout(alignment: .leading, spacing: 2)) : AnyLayout(HStackLayout(spacing: 8))
                    HStack(spacing: 8) {
                        row {
                            label(L.readerReadingMode)
                            if !big { Spacer(minLength: 8) }
                            Text(modeValue).font(.ui(15)).foregroundStyle(theme.haze)
                                .multilineTextAlignment(big ? .leading : .trailing)
                        }
                        if big { Spacer(minLength: 8) }
                        Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(theme.haze)
                            .accessibilityHidden(true)
                    }
                    .frame(minHeight: 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityElement(children: .combine)
                if model.modeOverridden {
                    let note = big ? AnyLayout(VStackLayout(alignment: .leading, spacing: 4)) : AnyLayout(HStackLayout(alignment: .firstTextBaseline, spacing: 6))
                    note {
                        Text(L.readerModeOverrideNote(title: model.view?.title ?? "")).font(.ui(13)).foregroundStyle(theme.haze)
                        Button(L.readerUseDefault) { model.useDefault(.mode); model.useDefault(.direction) }
                            .buttonStyle(.plain).font(.ui(13, weight: .semibold)).foregroundStyle(theme.silk).underline()
                    }
                } else if model.row(.direction)?.detected == true {
                    Text(L.readerModeDetectedNote).font(.ui(13)).foregroundStyle(theme.haze)
                }
            }
        }
    }

    /// The row's value: "Auto · Right to left" while guessed, the choice otherwise.
    private var modeValue: String {
        let guessed = model.choice(.mode) == "strip" ? L.readerModeStrip
            : (model.choice(.direction) == "ltr" ? L.readerModeLtr : L.readerModeRtl)
        switch currentMode {
        case .auto: return L.readerModeAuto(mode: guessed)
        case .rightToLeft: return L.readerModeRtl
        case .leftToRight: return L.readerModeLtr
        case .strip: return L.readerModeStrip
        }
    }

    /// The pushed picker: back, the series, the four modes with a check, and where it is kept.
    private var modePicker: some View {
        let options: [(String, String?, ReaderModel.ReadingModeChoice)] = [
            (autoName, L.readerModeAutoDetail, .auto), (L.readerModeRtl, nil, .rightToLeft),
            (L.readerModeLtr, nil, .leftToRight), (L.readerModeStrip, L.readerModeStripDetail, .strip),
        ]
        return scrolling {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 4) {
                    Button {
                        withAnimation(.standard) { pickingMode = false }
                    } label: {
                        Image(systemName: "chevron.left").font(.system(size: 17, weight: .semibold))
                            .frame(width: 44, height: 44).contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.readerSettingsOpen)
                    Text(L.readerReadingMode).font(.ui(17, weight: .semibold))
                        .accessibilityAddTraits(.isHeader)
                }
                .padding(.leading, -12)
                header(model.view?.title ?? "").padding(.top, 6).padding(.bottom, 4)
                ForEach(0..<options.count, id: \.self) { index in
                    let option = options[index]
                    let on = currentMode == option.2
                    Button { model.chooseMode(option.2) } label: {
                        HStack(spacing: 8) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(option.0).font(.ui(15, weight: .medium)).foregroundStyle(theme.silk)
                                if let detail = option.1 {
                                    Text(detail).font(.ui(13)).foregroundStyle(theme.haze)
                                }
                            }
                            Spacer(minLength: 8)
                            if on {
                                Image(systemName: "checkmark").font(.system(size: 15, weight: .semibold))
                                    .foregroundStyle(theme.silk)
                            }
                        }
                        .padding(.vertical, 8)
                        .frame(minHeight: 44)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityElement(children: .combine)
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
                Text(L.readerModeChoiceNote).font(.ui(13)).foregroundStyle(theme.haze).padding(.top, 10)
            }
        }
    }

    private var autoName: String { L.readerModeAutoOption }

    private var currentMode: ReaderModel.ReadingModeChoice {
        guard model.modeOverridden else { return .auto }
        if model.choice(.mode) == "strip" { return .strip }
        return model.choice(.direction) == "ltr" ? .leftToRight : .rightToLeft
    }

    @ViewBuilder private var paperSection: some View {
        if model.row(.paperMode) != nil {
            VStack(alignment: .leading, spacing: 12) {
                header(L.readerGroupPaper)
                toggle(L.readerPaperMode, .paperMode)
                Text(L.readerPaperModeNote).font(.ui(13)).foregroundStyle(theme.haze)
                if model.row(.greyscalePages) != nil, !novel { toggle(L.readerGreyscale, .greyscalePages) }
                if model.row(.quietPhone) != nil, ReaderDevice.isPhone {
                    toggle(L.readerQuiet, .quietPhone)
                    Text(L.readerPaperQuietNote).font(.ui(13)).foregroundStyle(theme.haze)
                }
            }
        }
    }

    private var moreSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            Button {
                withAnimation(.standard) { more.toggle() }
            } label: {
                HStack {
                    Text(L.readerMoreSettings).font(.ui(15, weight: .semibold))
                    Spacer()
                    Image(systemName: "chevron.down").rotationEffect(.degrees(more ? 180 : 0)).font(.system(size: 13))
                }
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            if more {
                TextField(L.readerSearchSettings, text: $query).textFieldStyle(.roundedBorder)
                if show(L.readerPageLayout), !novel {
                    segments(L.readerPageLayout, [(L.readerLayoutAuto, "auto"), (L.readerLayoutSingle, "single"), (L.readerLayoutDouble, "double")], .pageLayout)
                }
                if show(L.readerSpreads), !novel {
                    segments(L.readerSpreads, [(L.readerSpreadsSplit, "split"), (L.readerSpreadsRotate, "rotate"), (L.readerSpreadsWhole, "whole")], .spreads)
                }
                if show(L.readerTapZones) {
                    segments(L.readerTapZones, [(L.readerTapEdges, "edges"), (L.readerTapSides, "sides"), (L.readerTapOff, "off")], .tapLayout)
                }
                if show(L.readerStripGap), !novel, model.view?.mode == .strip {
                    chips(L.readerStripGap, [(L.readerGapNone, "none"), (L.readerGapSmall, "small")], gap) { gap = $0 }
                }
                if show(L.readerKeyStep), mac {
                    chips(L.readerKeyStep, [(L.readerKeyStepPanel, "panel"), (L.readerKeyStepScreen, "screen")], step) { step = $0 }
                }
                if show(L.readerVolumeKeys), model.row(.volumeKeysTurn) != nil { toggle(L.readerVolumeKeys, .volumeKeysTurn) }
            }
        }
    }

    private func show(_ name: String) -> Bool {
        query.isEmpty || name.localizedCaseInsensitiveContains(query)
    }

    // MARK: Controls

    private func header(_ text: String) -> some View {
        Text(text).font(.ui(13, weight: .semibold)).foregroundStyle(theme.haze).textCase(nil)
    }

    private func label(_ text: String) -> some View { Text(text).font(.ui(15, weight: .medium)) }

    /// A row of options for one engine choice.
    @ViewBuilder private func segments(
        _ title: String, _ options: [(String, String)], _ key: ReaderSettingKey, custom: [(String, String)]? = nil
    ) -> some View {
        if model.row(key) != nil {
            let list = custom ?? options
            chips(title, list, model.choice(key) ?? "") { model.set(key, .choice(name: $0)) }
        }
    }

    private func chips<V: Hashable>(
        _ title: String, _ options: [(String, V)], _ current: V, set: @escaping (V) -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            label(title)
            if big {
                checkList(options, current, set: set)
            } else {
                chipRow(options, current, set: set)
            }
        }
    }

    /// The choices as a list with a check, as the system pickers have it, for the largest sizes.
    private func checkList<V: Hashable>(_ options: [(String, V)], _ current: V, set: @escaping (V) -> Void) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(0..<options.count, id: \.self) { index in
                let on = options[index].1 == current
                Button { set(options[index].1) } label: {
                    HStack(spacing: 8) {
                        Text(options[index].0).font(.ui(15)).foregroundStyle(theme.silk)
                            .fixedSize(horizontal: false, vertical: true)
                        Spacer(minLength: 8)
                        if on { Image(systemName: "checkmark").font(.system(size: 15, weight: .semibold)) }
                    }
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .frame(minHeight: 44)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
                if index < options.count - 1 { Rectangle().fill(theme.hair).frame(height: 1).padding(.leading, 14) }
            }
        }
        .background(theme.raised, in: .rect(cornerRadius: 14))
    }

    private func chipRow<V: Hashable>(_ options: [(String, V)], _ current: V, set: @escaping (V) -> Void) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                ForEach(0..<options.count, id: \.self) { index in
                    let on = options[index].1 == current
                    Button { set(options[index].1) } label: {
                        Text(options[index].0).font(.ui(14, weight: .medium))
                            .lineLimit(1).minimumScaleFactor(0.75)
                            .frame(maxWidth: .infinity, minHeight: 44)
                            .foregroundStyle(on ? theme.silk : theme.haze)
                            .background(on ? theme.selected : Color.clear, in: .capsule)
                            .contentShape(.capsule)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
            }
            .padding(3)
            .background(theme.raised, in: .capsule)
        }
    }

    @ViewBuilder private func toggle(_ title: String, _ key: ReaderSettingKey) -> some View {
        if model.row(key) != nil {
            Toggle(isOn: Binding(get: { model.isOn(key) }, set: { model.set(key, .toggle(on: $0)) })) {
                label(title)
            }
            .tint(theme.water)
        }
    }

    @ViewBuilder private func stepper(_ title: String, _ key: ReaderSettingKey, _ by: Double, _ range: ClosedRange<Double>) -> some View {
        if let value = model.amount(key) {
            HStack {
                label(title)
                Spacer()
                Button { model.set(key, .amount(value: max(range.lowerBound, value - by))) } label: {
                    Image(systemName: "minus").frame(width: 38, height: 38).background(theme.raised, in: .circle)
                }
                Text("\(Int((value * 100).rounded()))%").font(.ui(15)).monospacedDigit().frame(minWidth: 52)
                Button { model.set(key, .amount(value: min(range.upperBound, value + by))) } label: {
                    Image(systemName: "plus").frame(width: 38, height: 38).background(theme.raised, in: .circle)
                }
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder private func amounts(_ title: String, _ key: ReaderSettingKey, _ options: [(String, Double)]) -> some View {
        if let value = model.amount(key) {
            let nearest = options.min { abs($0.1 - value) < abs($1.1 - value) }?.1 ?? value
            chips(title, options, nearest) { model.set(key, .amount(value: $0)) }
        }
    }
}

enum ReaderDevice {
    static var isPhone: Bool {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .phone
        #else
        false
        #endif
    }

    static var isPad: Bool {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .pad
        #else
        false
        #endif
    }
}

// MARK: Chapters and pages

struct ReaderChaptersSheet: View {
    @Bindable var model: ReaderModel
    let theme: ReaderTheme
    @State private var tab = 0
    /// The loading line waits 600 ms, as every state does.
    @State private var loadingShown = false

    private var hasPages: Bool { model.medium == .manga }
    /// Pages a jump in the grid covers.
    private static let band = 20

    var body: some View {
        VStack(spacing: 0) {
            if hasPages {
                Picker("", selection: $tab) {
                    Text(L.readerPagesTab).tag(0)
                    Text(L.readerChaptersTab).tag(1)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .padding(16)
            }
            if hasPages && tab == 0 { pages } else { chapters }
        }
        .frame(width: ReaderDevice.isPhone ? nil : 520, height: ReaderDevice.isPhone ? nil : 520)
        .presentationDetents([.fraction(0.55), .large])
        .presentationBackgroundInteraction(.enabled)
        .presentationBackground(theme.glassSolid)
        .presentationCompactAdaptation(.sheet)
        .preferredColorScheme(theme.scheme)
        .foregroundStyle(theme.silk)
    }

    /// Every page of the chapter, as the person counts them: both halves of a spread are here.
    private var pageList: [(number: Int, leaf: Int, part: LeafPart)] {
        var all: [(Int, Int, LeafPart)] = []
        for (index, leaf) in model.leaves.enumerated() where !leaf.endPage {
            for part in leaf.parts { all.append((Int(part.page) + 1, index, part)) }
        }
        return all.sorted { $0.0 < $1.0 }
    }

    private var pages: some View {
        let rtl = model.view?.direction == .rightToLeft
        let columns = ReaderDevice.isPhone
            ? Array(repeating: GridItem(.flexible(), spacing: 10), count: 4)
            : Array(repeating: GridItem(.fixed(84), spacing: 10), count: 5)
        let list = pageList
        return ScrollViewReader { proxy in
            VStack(spacing: 0) {
                // A long chapter folds into bands of 20 pages, one tap from each.
                if list.count > Self.band * 2 {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(Array(stride(from: 1, through: list.count, by: Self.band)), id: \.self) { first in
                                let last = min(first + Self.band - 1, list.count)
                                Button(L.readerPagesRange(from: first, to: last)) {
                                    withAnimation(.standard) { proxy.scrollTo(first, anchor: .top) }
                                }
                                .font(.ui(13, weight: .medium)).monospacedDigit()
                                .foregroundStyle(theme.silk)
                                .padding(.horizontal, 12)
                                .frame(minHeight: 32)
                                .background(theme.raised, in: .capsule)
                                .buttonStyle(.plain)
                            }
                        }
                        .padding(.horizontal, 16)
                    }
                    .padding(.bottom, 10)
                }
                pageGrid(list, columns: columns, rtl: rtl)
            }
        }
    }

    private func pageGrid(_ list: [(number: Int, leaf: Int, part: LeafPart)], columns: [GridItem], rtl: Bool) -> some View {
        ScrollView {
            LazyVGrid(columns: columns, spacing: 10) {
                ForEach(list, id: \.number) { entry in
                    Button {
                        model.requestJump(entry.leaf)
                        model.sheet = nil
                    } label: {
                        thumb(entry.part, number: entry.number, open: entry.leaf == model.current)
                    }
                    .buttonStyle(.plain)
                    .id(entry.number)
                    .environment(\.layoutDirection, .leftToRight)
                }
            }
            .padding(.horizontal, 16).padding(.bottom, 16)
            // A right-to-left series lays its grid out from the right.
            .environment(\.layoutDirection, rtl ? .rightToLeft : .leftToRight)
        }
    }

    private func thumb(_ part: LeafPart, number: Int, open: Bool) -> some View {
        let chapter = model.view?.chapter.id ?? ""
        let file = model.file(of: part, chapter: chapter)
        return VStack(spacing: 4) {
            ZStack {
                Rectangle().fill(theme.raised)
                if let file { PageImage(file: file, longest: 240, fill: true) }
            }
            .aspectRatio(0.7, contentMode: .fit)
            .clipShape(.rect(cornerRadius: 8))
            .overlay {
                if open { RoundedRectangle(cornerRadius: 8).stroke(theme.silk, lineWidth: 2) }
            }
            Text("\(number)").font(.ui(13)).monospacedDigit().foregroundStyle(theme.haze)
        }
        .onAppear { if file == nil { model.needPage(part.page) } }
    }

    private var chapters: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if !model.chaptersLoaded, loadingShown {
                    Text(L.readerChaptersLoading).font(.ui(15)).foregroundStyle(theme.haze).padding(16)
                        .transition(.opacity)
                }
                ForEach(0..<model.chapters.count, id: \.self) { group in
                    let volume = model.chapters[group]
                    if let name = volume.title ?? volume.volume.map({ L.readerVolume(volume: $0) }) {
                        Text(name).font(.ui(13, weight: .semibold)).foregroundStyle(theme.haze).textCase(nil)
                            .padding(.horizontal, 16).padding(.top, 14).padding(.bottom, 6)
                    }
                    ForEach(0..<volume.chapters.count, id: \.self) { row in chapterRow(volume.chapters[row]) }
                }
            }
        }
        .task {
            try? await Task.sleep(for: .milliseconds(600))
            withAnimation(.fadeIn) { loadingShown = true }
        }
    }

    private func chapterRow(_ row: ChapterRow) -> some View {
        let here = row.id == model.view?.chapter.id
        return Button {
            if !here { model.openChapter(row.id) }
            model.sheet = nil
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.title ?? row.label).font(.ui(16, weight: here ? .semibold : .regular))
                        .foregroundStyle(rowRead(row) ? theme.haze : theme.silk).lineLimit(1)
                    Text(state(row)).font(.ui(13)).foregroundStyle(theme.haze)
                }
                Spacer()
            }
            .padding(.horizontal, 16).padding(.vertical, 10)
            .background(here ? theme.selected : Color.clear)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(here ? .isSelected : [])
    }

    private func rowRead(_ row: ChapterRow) -> Bool {
        if case .read = row.state { return true }
        return false
    }

    private func state(_ row: ChapterRow) -> String {
        switch row.state {
        case .read: L.readerChapterRead
        case .inProgress(let page, let of): L.readerChapterInProgress(page: Int(page), total: Int(of ?? 0))
        case .unread: row.newChapter ? L.readerChapterNew : (row.pages.map { L.readerChapterPages(pages: Int($0)) } ?? row.label)
        }
    }
}
