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

    private var novel: Bool { model.medium == .novel }

    var body: some View {
        ScrollView {
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
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .scrollIndicators(.hidden)
        .frame(width: mac || ReaderDevice.isPad ? 380 : nil, height: mac || ReaderDevice.isPad ? 560 : nil)
        .presentationDetents([.fraction(0.55), .large])
        .presentationBackgroundInteraction(.enabled)
        .presentationBackground(theme.glassSolid)
        .presentationCompactAdaptation(.sheet)
        .preferredColorScheme(system ? nil : theme.scheme)
        .foregroundStyle(theme.silk)
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
            add(.fit, segments(L.readerFit, [(L.readerFitScreen, "screen"), (L.readerFitWidth, "width")], .fit))
            add(.cropBorders, toggle(L.readerCropBorders, .cropBorders))
            rows.append((.device, item("background", chips(
                L.readerBackground, [(L.readerBackgroundTheme, "theme"), (L.readerBackgroundBlack, "black"), (L.readerBackgroundWhite, "white")],
                background) { background = $0 })))
        }
        let strip = model.view?.mode == .strip
        let medium = novel ? L.readerAllNovels : (strip ? L.readerAllStrips : L.readerAllManga)
        let device = novel ? medium : L.readerAllMangaHere
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
                let current = currentMode
                chips(
                    L.readerReadingMode,
                    [
                        (autoName, .auto), (L.readerModeRtl, .rightToLeft), (L.readerModeLtr, .leftToRight),
                        (L.readerModeStrip, .strip),
                    ], current
                ) { model.chooseMode($0) }
                if model.modeOverridden {
                    Button(L.readerUseDefault) { model.useDefault(.mode); model.useDefault(.direction) }
                        .buttonStyle(.plain).font(.ui(13, weight: .semibold)).foregroundStyle(theme.haze).underline()
                } else if model.row(.direction)?.detected == true {
                    Text(L.readerModeDetectedNote).font(.ui(13)).foregroundStyle(theme.haze)
                }
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
                if show(L.readerStripWidth), !novel, model.view?.mode == .strip, mac || ReaderDevice.isPad {
                    stepper(L.readerStripWidth, .stripWidth, 0.1, 0.3...1.0)
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

    private var hasPages: Bool { model.medium == .manga }

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
        return ScrollView {
            LazyVGrid(columns: columns, spacing: 10) {
                ForEach(pageList, id: \.number) { entry in
                    Button {
                        model.requestJump(entry.leaf)
                        model.sheet = nil
                    } label: {
                        thumb(entry.part, number: entry.number, open: entry.leaf == model.current)
                    }
                    .buttonStyle(.plain)
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
                if !model.chaptersLoaded {
                    Text(L.readerChaptersLoading).font(.ui(15)).foregroundStyle(theme.haze).padding(16)
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
                if here { Image(systemName: "book.fill").foregroundStyle(theme.silk) }
            }
            .padding(.horizontal, 16).padding(.vertical, 10)
            .background(here ? theme.selected : Color.clear)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
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
