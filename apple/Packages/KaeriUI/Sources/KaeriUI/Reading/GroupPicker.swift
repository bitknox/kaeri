// The group picker: whose version of each chapter a series reads, inside the source in use. A
// popover pinned to the Group row's Change on the Mac and iPad, a medium sheet with a close
// button on iPhone. Most chapters comes first and names the group it means today; then each
// group with its chapters out of the series' and its language. A pick closes it.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct GroupPicker: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var type
    let work: String
    let edition: BookEditionRow
    /// The row the arrow keys are on, on the Mac and an iPad with a keyboard: 0 is Most chapters.
    @State private var keyed: Int?
    @FocusState private var focused: Bool

    private var phone: Bool { ReaderDevice.isPhone }
    private var mac: Bool { Device.isMac }

    /// What Most chapters picks today: the group in use while nothing is picked, else the first
    /// in the app's language, else the one with the most chapters.
    static func mostNow(_ edition: BookEditionRow) -> String? {
        if !edition.groupChosen {
            return edition.groups.first(where: \.inUse)?.label ?? edition.versionLabel
        }
        let app = AirTime.locale.language.languageCode?.identifier
        let own = edition.groups.first { group in
            group.language.map { ReadWords.primary($0) } == app
        }
        return (own ?? edition.groups.first)?.label
    }

    /// The series' chapter count, which each group's is out of.
    private var total: Int {
        max(Int(edition.chapters), edition.groups.map { Int($0.chapters) }.max() ?? 0)
    }

    var body: some View {
        if phone { sheet } else { popover }
    }

    private static var closePlacement: ToolbarItemPlacement {
        #if os(iOS)
        .topBarTrailing
        #else
        .cancellationAction
        #endif
    }

    // MARK: iPhone

    private var sheet: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    SheetRows { row(nil, index: 0) }
                    SheetRows {
                        ForEach(Array(edition.groups.enumerated()), id: \.element.label) { n, group in
                            row(group, index: n + 1)
                        }
                    }
                    SheetNote(text: L.groupPickerFallback).padding(.top, -8)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .background(.raised)
            .navigationBarTitleDisplayModeInline()
            .toolbar {
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 0) {
                        Text(L.seriesGroup).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                        Text(L.groupPickerIn(source: edition.sourceName)).font(.ui(12)).foregroundStyle(.haze)
                    }
                }
                // The system's quiet close, not a filled confirmation.
                ToolbarItem(placement: Self.closePlacement) {
                    Button(role: .close) { dismiss() }
                }
            }
        }
        // The short sheet; at accessibility sizes it opens at full height and its rows wrap.
        .presentationDetents(type.isAccessibilitySize ? [.large] : [.medium, .large])
        .presentationBackground(.raised)
    }

    // MARK: Mac and iPad

    private var popover: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 2) {
                Text(L.seriesGroup).font(.plat(mac: 13, touch: 17, weight: .semibold)).foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Text(L.groupPickerIn(source: edition.sourceName)).font(.plat(mac: 11, touch: 13)).foregroundStyle(.haze)
            }
            .padding(.horizontal, 14)
            .padding(.top, 12)
            .padding(.bottom, 6)
            row(nil, index: 0)
            rule
            ForEach(Array(edition.groups.enumerated()), id: \.element.label) { n, group in
                row(group, index: n + 1)
            }
            rule
            Text(L.groupPickerFallback)
                .font(.plat(mac: 11, touch: 13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
        }
        .frame(width: 300, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
        .background(.raised)
        .presentationCompactAdaptation(.popover)
        // Up and Down move along the rows, Return picks; Escape closes, as popovers do.
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onAppear { focused = true }
        .onKeyPress(.downArrow) { move(1) }
        .onKeyPress(.upArrow) { move(-1) }
        .onKeyPress(.return) {
            guard let keyed else { return .ignored }
            pick(keyed == 0 ? nil : edition.groups[keyed - 1])
            return .handled
        }
    }

    private var rule: some View {
        Rectangle().fill(Palette.faintRule).frame(height: 1).padding(.vertical, 4).accessibilityHidden(true)
    }

    private func move(_ by: Int) -> KeyPress.Result {
        let count = edition.groups.count + 1
        keyed = min(max((keyed ?? (by > 0 ? -1 : count)) + by, 0), count - 1)
        return .handled
    }

    // MARK: Rows

    private func checked(_ group: GroupChoice?) -> Bool {
        guard let group else { return !edition.groupChosen }
        return edition.groupChosen && group.inUse
    }

    /// Most chapters with nil, else a group: its name, and what it means or holds under it.
    private func row(_ group: GroupChoice?, index: Int) -> some View {
        let on = checked(group)
        let title = group?.label ?? L.groupPickerMost
        let line: String? = if let group {
            ([L.groupPickerCount(have: Int(group.chapters), total: total)] + [group.language.map(ReadWords.language)].compactMap { $0 })
                // A no-break space before the dot keeps it with the count when the line wraps.
                .joined(separator: "\u{00A0}· ") // i18n-skip
        } else {
            Self.mostNow(edition).map { L.groupPickerMostNow(group: $0) }
        }
        let check = Image(systemName: "checkmark")
            .font(.ui(mac ? 11 : 15, weight: .semibold))
            .foregroundStyle(.silk)
            .opacity(on ? 1 : 0)
            .accessibilityHidden(true)
        return Button { pick(group) } label: {
            HStack(spacing: mac ? 8 : 12) {
                // The Mac checks on the leading side, as its menus do; touch on the trailing side.
                if !phone { check.frame(width: 14) }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(.silk)
                    if let line {
                        Text(line)
                            .font(.plat(mac: 11, touch: 15))
                            .foregroundStyle(.haze)
                    }
                }
                .lineLimit(type.isAccessibilitySize ? nil : 1)
                Spacer(minLength: 8)
                if phone { check }
            }
            .padding(.horizontal, phone ? 12 : 14)
            .padding(.vertical, mac ? 5 : 8)
            .frame(minHeight: mac ? 36 : (phone ? 56 : 48), alignment: .leading)
            .background(keyed == index ? Palette.raisedSelected : .clear, in: .rect(cornerRadius: 6))
            .padding(.horizontal, phone ? 0 : 4)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func pick(_ group: GroupChoice?) {
        if !checked(group) { store.reading.chooseGroup(work, group: group?.label) }
        dismiss()
    }
}

extension ReadWords {
    /// A language tag's first part: "es" of "es-419".
    static func primary(_ tag: String) -> String {
        (tag.split { $0 == "-" || $0 == "_" }.first.map(String.init) ?? tag).lowercased()
    }
}

private extension View {
    @ViewBuilder func navigationBarTitleDisplayModeInline() -> some View {
        #if os(iOS)
        navigationBarTitleDisplayMode(.inline)
        #else
        self
        #endif
    }
}
