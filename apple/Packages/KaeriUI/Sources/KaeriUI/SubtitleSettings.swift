// Settings, Playback, Subtitles: the preview, then Follow system caption style, Size, Font, Text
// colour, Edge, Background, Position, Styled subtitles and Reset. The engine keeps the style for
// the device; every control sends one setting and the preview follows what the engine answers.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

// MARK: - Words and values

extension SubtitleFont {
    static let all: [SubtitleFont] = [.default, .zenKakuGothicNew, .serif, .monospace]
    var words: String {
        switch self {
        case .default: L.settingsSubsFontDefault
        case .zenKakuGothicNew: "Zen Kaku Gothic New" // i18n-skip: a typeface's name
        case .serif: L.settingsSubsFontSerif
        case .monospace: L.settingsSubsFontMono
        }
    }
}

extension SubtitleColour {
    static let all: [SubtitleColour] = [.white, .yellow, .cyan, .green]
    var words: String {
        switch self {
        case .white: L.settingsSubsColourWhite
        case .yellow: L.settingsSubsColourYellow
        case .cyan: L.settingsSubsColourCyan
        case .green: L.settingsSubsColourGreen
        }
    }
    var swatch: Color {
        switch self {
        case .white: .white
        case .yellow: Color(red: 1, green: 1, blue: 0)
        case .cyan: Color(red: 0, green: 1, blue: 1)
        case .green: Color(red: 0, green: 1, blue: 0)
        }
    }
}

extension SubtitleEdge {
    static let all: [SubtitleEdge] = [.outline, .dropShadow, .noEdge]
    var words: String {
        switch self {
        case .outline: L.settingsSubsEdgeOutline
        case .dropShadow: L.settingsSubsEdgeShadow
        case .noEdge: L.settingsSubsEdgeNone
        }
    }
}

extension SubtitleThickness {
    static let all: [SubtitleThickness] = [.thin, .medium, .thick]
    var words: String {
        switch self {
        case .thin: L.settingsSubsThin
        case .medium: L.settingsSubsMedium
        case .thick: L.settingsSubsThick
        }
    }
}

extension SubtitleBackground {
    static let all: [SubtitleBackground] = [.noBackground, .eachLine, .allLines]
    var words: String {
        switch self {
        case .noBackground: L.settingsSubsBackgroundNone
        case .eachLine: L.settingsSubsBackgroundLine
        case .allLines: L.settingsSubsBackgroundAll
        }
    }
}

extension StyledSubtitles {
    static let all: [StyledSubtitles] = [.keep, .restyle, .strip]
    var words: String {
        switch self {
        case .keep: L.settingsSubsStyledKeep
        case .restyle: L.settingsSubsStyledRestyle
        case .strip: L.settingsSubsStyledStrip
        }
    }
    /// The line under the picker for this choice.
    var detail: String {
        switch self {
        case .keep: L.settingsSubsStyledKeepDetail
        case .restyle: L.settingsSubsStyledRestyleDetail
        case .strip: L.settingsSubsStyledStripDetail
        }
    }
}

extension AppStore {
    /// Send one subtitle setting; the engine keeps it for the device.
    func setSubtitle(_ key: SettingKey, int value: Int) {
        setSetting(key, to: .int(Int64(value)))
    }

    func setSubtitle(_ key: SettingKey, name value: String) {
        setSetting(key, to: .str(value))
    }
}

// MARK: - The controls, shared by the Mac, iPad and iPhone

/// A slider over a stepped range, with its value at the end. Sends once per step.
struct StepSlider: View {
    let title: String
    let range: ClosedRange<Int>
    let step: Int
    let value: Int
    let words: (Int) -> String
    let commit: (Int) -> Void

    var body: some View {
        HStack(spacing: 12) {
            Slider(
                value: Binding(get: { Double(value) }, set: { commit(Int($0)) }),
                in: Double(range.lowerBound)...Double(range.upperBound), step: Double(step)
            )
            .tint(.silk)
            .frame(minWidth: 120, maxWidth: 220)
            .accessibilityLabel(title)
            .accessibilityValue(words(value))
            Text(words(value))
                .font(.mac(13))
                .monospacedDigit()
                .foregroundStyle(.silk)
                .frame(minWidth: 56, alignment: .trailing)
        }
    }
}

/// Text colour as swatches with their names: colour is never the only signal.
struct ColourSwatches: View {
    let current: SubtitleColour
    let choose: (SubtitleColour) -> Void
    var enabled = true

    var body: some View {
        HStack(spacing: 6) {
            ForEach(SubtitleColour.all, id: \.self) { colour in
                Button { choose(colour) } label: {
                    Circle()
                        .fill(colour.swatch)
                        .frame(width: 18, height: 18)
                        .overlay(Circle().strokeBorder(Palette.glassRule, lineWidth: 1))
                        .padding(4)
                        .overlay(Circle().strokeBorder(Color.silk, lineWidth: current == colour ? 2 : 0))
                        .frame(minWidth: 32, minHeight: 32)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .disabled(!enabled)
                .accessibilityLabel(colour.words)
                .accessibilityAddTraits(current == colour ? .isSelected : [])
            }
        }
    }
}

/// A menu over a closed set, in the capsule the other settings menus use.
struct ChoiceMenu<Choice: Hashable>: View {
    let title: String
    let choices: [Choice]
    let current: Choice
    let words: (Choice) -> String
    let choose: (Choice) -> Void
    var enabled = true

    var body: some View {
        Menu {
            Picker(title, selection: Binding(get: { current }, set: choose)) {
                ForEach(choices, id: \.self) { Text(words($0)).tag($0) }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuCapsule(text: enabled ? words(current) : L.settingsSubsSystemValue)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(!enabled)
        .accessibilityLabel(title)
    }
}

/// The opacity ladder and the thickness, as the engine steps them.
enum SubtitleLadders {
    static let opacities = [50, 75, 100]
}

// MARK: - Mac and iPad

/// The Subtitles group in Settings, Playback.
struct SubtitleStyleGroup: View {
    @Environment(AppStore.self) private var store
    @State private var kind = SubtitleKind.plain
    #if os(macOS)
    @Environment(\.openURL) private var openURL
    #endif

    var body: some View {
        let style = store.settings.subtitleStyle
        let system = style.followSystem
        SettingsGroup(title: L.playerSubtitles) {
            preview(style)
            SettingsRow(
                title: L.settingsSystemCaptions,
                note: L.settingsSystemCaptionsDetail
            ) {
                Toggle(L.settingsSystemCaptions, isOn: Binding(
                    get: { style.followSystem },
                    set: { store.setSwitch(.followSystemCaptions, on: $0) }
                ))
                .toggleStyle(.switch)
                .tint(.amber)
                .labelsHidden()
            }
            SettingsRow(title: L.settingsSubsSize) {
                StepSlider(
                    title: L.settingsSubsSize, range: 50...200, step: 10, value: Int(style.size),
                    words: { L.settingsSubsSizeValue(percent: $0) },
                    commit: { store.setSubtitle(.subtitleSize, int: $0) })
            }
            SettingsRow(title: L.settingsSubsFont, note: system ? systemNote : nil) {
                ChoiceMenu(
                    title: L.settingsSubsFont, choices: SubtitleFont.all, current: style.font,
                    words: \.words, choose: { store.setSubtitle(.subtitleFont, name: $0.fileName) },
                    enabled: !system)
            }
            .opacity(system ? 0.55 : 1)
            SettingsRow(title: L.settingsSubsColour) {
                ColourSwatches(current: style.colour, choose: { store.setSubtitle(.subtitleColour, name: $0.fileName) }, enabled: !system)
            }
            .opacity(system ? 0.55 : 1)
            SettingsRow(title: L.settingsSubsEdge) {
                HStack(spacing: 8) {
                    if style.edge == .outline, !system {
                        ChoiceMenu(
                            title: L.settingsSubsThickness, choices: SubtitleThickness.all, current: style.thickness,
                            words: \.words, choose: { store.setSubtitle(.subtitleThickness, name: $0.fileName) })
                    }
                    ChoiceMenu(
                        title: L.settingsSubsEdge, choices: SubtitleEdge.all, current: style.edge,
                        words: \.words, choose: { store.setSubtitle(.subtitleEdge, name: $0.fileName) },
                        enabled: !system)
                }
            }
            .opacity(system ? 0.55 : 1)
            SettingsRow(title: L.settingsSubsBackground) {
                HStack(spacing: 8) {
                    if style.background != .noBackground, !system {
                        ChoiceMenu(
                            title: L.settingsSubsOpacity, choices: SubtitleLadders.opacities, current: Int(style.opacity),
                            words: { L.settingsSubsSizeValue(percent: $0) },
                            choose: { store.setSubtitle(.subtitleOpacity, int: $0) })
                    }
                    ChoiceMenu(
                        title: L.settingsSubsBackground, choices: SubtitleBackground.all, current: style.background,
                        words: \.words, choose: { store.setSubtitle(.subtitleBackground, name: $0.fileName) },
                        enabled: !system)
                }
            }
            .opacity(system ? 0.55 : 1)
            SettingsRow(title: L.settingsSubsRaise) {
                StepSlider(
                    title: L.settingsSubsRaise, range: 0...20, step: 1, value: Int(style.raise),
                    words: { L.settingsSubsRaiseValue(percent: $0) },
                    commit: { store.setSubtitle(.subtitleRaise, int: $0) })
            }
            SettingsRow(title: L.settingsSubsStyled, note: style.styled.detail) {
                ChoiceMenu(
                    title: L.settingsSubsStyled, choices: StyledSubtitles.all, current: style.styled,
                    words: \.words, choose: { store.setSubtitle(.styledSubtitles, name: $0.fileName) })
            }
            SettingsRow(title: L.settingsSubsReset) {
                Button(L.settingsSubsReset) { store.resetSubtitleStyle() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
    }

    /// The board's Mac frame: the preview at the row inset with the label and the Plain | Styled
    /// picker beside it. Where the row is too narrow for both, the picker goes above the preview.
    private func preview(_ style: SubtitleStyle) -> some View {
        let picker = Picker(L.settingsSubsPreview, selection: $kind) {
            Text(L.settingsSubsPreviewPlain).tag(SubtitleKind.plain)
            Text(L.settingsSubsPreviewStyled).tag(SubtitleKind.styled)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .fixedSize()
        let frame = SubtitlePreview(
            style: style, kind: kind, system: style.followSystem ? SystemCaptions.current() : nil, width: 320)
        return ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 20) {
                frame
                VStack(alignment: .leading, spacing: 8) {
                    Text(L.settingsSubsPreview).font(.mac(13)).foregroundStyle(.silk)
                    picker
                }
                Spacer(minLength: 0)
            }
            VStack(alignment: .leading, spacing: 10) {
                picker
                frame
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        // The row labels start 14 in (4 row margin plus 10 row padding); the preview shares that edge.
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .transaction { $0.animation = nil }
    }

    /// "From your system", with the link that opens the settings it comes from.
    private var systemNote: String { L.settingsSubsSystemChange }
}

// MARK: - iPhone

#if os(iOS)
/// Settings, Playback, Subtitles on iPhone: the preview in its own section at the full row width,
/// then the controls as list rows.
struct PhoneSubtitleSections: View {
    @Environment(AppStore.self) private var store
    @State private var kind = SubtitleKind.plain

    var body: some View {
        let style = store.settings.subtitleStyle
        let system = style.followSystem
        Section {
            VStack(spacing: 10) {
                Picker(L.settingsSubsPreview, selection: $kind) {
                    Text(L.settingsSubsPreviewPlain).tag(SubtitleKind.plain)
                    Text(L.settingsSubsPreviewStyled).tag(SubtitleKind.styled)
                }
                .pickerStyle(.segmented)
                GeometryReader { proxy in
                    SubtitlePreview(
                        style: style, kind: kind, system: system ? SystemCaptions.current() : nil,
                        width: proxy.size.width)
                }
                .aspectRatio(16 / 9, contentMode: .fit)
            }
            .transaction { $0.animation = nil }
            .listRowBackground(Palette.raised)
            Toggle(isOn: Binding(get: { system }, set: { store.setSwitch(.followSystemCaptions, on: $0) })) {
                PhoneRowTitle(title: L.settingsSystemCaptions, note: L.settingsSystemCaptionsDetail)
            }
            .tint(.amber)
            .listRowBackground(Palette.raised)
            if system {
                Button(L.settingsSubsSystemChange) {
                    if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                }
                .font(.ui(15))
                .foregroundStyle(.silk)
                .listRowBackground(Palette.raised)
            }
            slider(L.settingsSubsSize, 50...200, 10, Int(style.size), { L.settingsSubsSizeValue(percent: $0) }) {
                store.setSubtitle(.subtitleSize, int: $0)
            }
            menu(L.settingsSubsFont, SubtitleFont.all, style.font, \.words, off: system) { store.setSubtitle(.subtitleFont, name: $0.fileName) }
            VStack(alignment: .leading, spacing: 6) {
                Text(L.settingsSubsColour).font(.ui(17)).foregroundStyle(.silk)
                HStack(spacing: 4) {
                    ForEach(SubtitleColour.all, id: \.self) { colour in
                        Button { store.setSubtitle(.subtitleColour, name: colour.fileName) } label: {
                            VStack(spacing: 4) {
                                Circle().fill(colour.swatch).frame(width: 26, height: 26)
                                    .overlay(Circle().strokeBorder(Color.silk, lineWidth: style.colour == colour ? 2.5 : 0).padding(-4))
                                Text(colour.words).font(.ui(12)).foregroundStyle(.haze)
                            }
                            .frame(maxWidth: .infinity, minHeight: 56)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .disabled(system)
                        .accessibilityLabel(colour.words)
                        .accessibilityAddTraits(style.colour == colour ? .isSelected : [])
                    }
                }
            }
            .opacity(system ? 0.55 : 1)
            .listRowBackground(Palette.raised)
            menu(L.settingsSubsEdge, SubtitleEdge.all, style.edge, \.words, off: system) { store.setSubtitle(.subtitleEdge, name: $0.fileName) }
            if style.edge == .outline, !system {
                menu(L.settingsSubsThickness, SubtitleThickness.all, style.thickness, \.words) {
                    store.setSubtitle(.subtitleThickness, name: $0.fileName)
                }
            }
            menu(L.settingsSubsBackground, SubtitleBackground.all, style.background, \.words, off: system) {
                store.setSubtitle(.subtitleBackground, name: $0.fileName)
            }
            if style.background != .noBackground, !system {
                menu(L.settingsSubsOpacity, SubtitleLadders.opacities, Int(style.opacity), { L.settingsSubsSizeValue(percent: $0) }) {
                    store.setSubtitle(.subtitleOpacity, int: $0)
                }
            }
            slider(L.settingsSubsRaise, 0...20, 1, Int(style.raise), { L.settingsSubsRaiseValue(percent: $0) }) {
                store.setSubtitle(.subtitleRaise, int: $0)
            }
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(L.settingsSubsStyled).font(.ui(17)).foregroundStyle(.silk)
                    Spacer()
                    Menu {
                        Picker(L.settingsSubsStyled, selection: Binding(get: { style.styled }, set: { store.setSubtitle(.styledSubtitles, name: $0.fileName) })) {
                            ForEach(StyledSubtitles.all, id: \.self) { Text($0.words).tag($0) }
                        }
                        .pickerStyle(.inline)
                    } label: { PhoneMenuValue(text: style.styled.words) }
                }
                Text(style.styled.detail).font(.ui(13)).foregroundStyle(.haze)
            }
            .listRowBackground(Palette.raised)
            Button(L.settingsSubsReset) { store.resetSubtitleStyle() }
                .font(.ui(17))
                .foregroundStyle(.silk)
                .listRowBackground(Palette.raised)
        } header: {
            PhoneSectionHeader(L.playerSubtitles)
        }
    }

    private func slider(
        _ title: String, _ range: ClosedRange<Int>, _ step: Int, _ value: Int,
        _ words: @escaping (Int) -> String, _ commit: @escaping (Int) -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title).font(.ui(17)).foregroundStyle(.silk)
                Spacer()
                Text(words(value)).font(.ui(17)).monospacedDigit().foregroundStyle(.haze)
            }
            Slider(
                value: Binding(get: { Double(value) }, set: { commit(Int($0)) }),
                in: Double(range.lowerBound)...Double(range.upperBound), step: Double(step)
            )
            .tint(.silk)
            .accessibilityLabel(title)
            .accessibilityValue(words(value))
        }
        .listRowBackground(Palette.raised)
    }

    private func menu<Choice: Hashable>(
        _ title: String, _ choices: [Choice], _ current: Choice, _ words: @escaping (Choice) -> String,
        off: Bool = false, _ choose: @escaping (Choice) -> Void
    ) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer()
            Menu {
                Picker(title, selection: Binding(get: { current }, set: choose)) {
                    ForEach(choices, id: \.self) { Text(words($0)).tag($0) }
                }
                .pickerStyle(.inline)
            } label: {
                PhoneMenuValue(text: off ? L.settingsSubsSystemValue : words(current))
            }
            .disabled(off)
        }
        .opacity(off ? 0.55 : 1)
        .listRowBackground(Palette.raised)
    }
}
#endif
