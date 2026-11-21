// The panel's Episodes tab: the title's episode table, as the title page has it, with the
// playing row marked. A first tap or click reveals a row's whole synopsis and what can be done
// with it; Play then plays it. On the Mac a double-click plays at once.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PanelEpisodes: View {
    let model: PlayerModel
    let metrics: PanelMetrics
    let close: () -> Void
    @Environment(AppStore.self) private var store
    /// The row revealed: its whole title, synopsis and buttons.
    @State private var revealed: String?
    /// The table has taken 4 s to come: the loading rows say so.
    @State private var slow = false

    var body: some View {
        let id = model.staged?.id
        let rows = id.flatMap { store.titles.episodes[$0] }
        let current = Self.current(model)
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    if let id, let rows {
                        let metadataOnly = rows.contains(where: \.metadataOnly)
                        if metadataOnly {
                            PanelText(
                                text: L.playerPanelMetadataOnly, size: metrics.detail, line: metrics.detailLine,
                                color: Palette.haze)
                                .padding(.horizontal, metrics.rowPad)
                                .padding(.top, 4)
                                .padding(.bottom, 12)
                        }
                        let spoilers = store.spoilerProtection(for: id)
                        LazyVStack(alignment: .leading, spacing: 2) {
                            ForEach(rows, id: \.number) { row in
                                PanelEpisodeRow(
                                    model: model, id: id, row: row, metrics: metrics,
                                    current: !metadataOnly && row.number == current,
                                    metadataOnly: metadataOnly,
                                    hidden: spoilers && row.spoilerAhead && row.number != current,
                                    revealed: revealed == row.number,
                                    reveal: { reveal(row.number) },
                                    played: close)
                                    .id(row.number)
                            }
                        }
                    } else {
                        loading
                    }
                }
                .padding(.horizontal, metrics.pad)
                .padding(.bottom, metrics.pad)
                .animation(.standard, value: revealed)
            }
            .scrollIndicators(.hidden)
            .task(id: rows == nil) {
                // The playing row near the top as the list opens, with the ones before it above.
                guard rows != nil, let current else { return }
                proxy.scrollTo(current, anchor: UnitPoint(x: 0.5, y: 0.25))
            }
        }
        .onChange(of: model.panelShot, initial: true) { _, shot in
            if let shot { revealed = shot.revealed }
        }
        .task(id: id) {
            guard let id else { return }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            slow = false
            try? await Task.sleep(for: .seconds(4))
            slow = true
        }
    }

    private func reveal(_ number: String) {
        revealed = revealed == number ? nil : number
    }

    /// Seven rows the shape of the ones to come, and a line once they take long.
    private var loading: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(0 ..< 7, id: \.self) { index in
                HStack(spacing: metrics.gap) {
                    RoundedRectangle(cornerRadius: metrics.stillRadius, style: .continuous)
                        .fill(Palette.silk.opacity(0.07))
                        .frame(width: metrics.still.width, height: metrics.still.height)
                    VStack(alignment: .leading, spacing: 8) {
                        bar([0.72, 0.58, 0.8, 0.64, 0.7, 0.52, 0.76][index], height: metrics.title - 2)
                        bar(0.34, height: metrics.detail - 3)
                    }
                }
                .padding(metrics.rowPad)
            }
            if slow {
                PanelText(text: L.playerSheetEpisodesLoading, size: metrics.detail, line: metrics.detailLine, color: Palette.haze)
                    .padding(.horizontal, metrics.rowPad)
                    .padding(.vertical, 8)
                    .transition(.opacity)
            }
        }
        .animation(.fadeIn, value: slow)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.playerSheetEpisodesLoading)
    }

    private func bar(_ fraction: CGFloat, height: CGFloat) -> some View {
        GeometryReader { geometry in
            RoundedRectangle(cornerRadius: 4, style: .continuous)
                .fill(Palette.silk.opacity(0.08))
                .frame(width: geometry.size.width * fraction)
        }
        .frame(height: height)
    }

    /// The episode playing, by number.
    static func current(_ model: PlayerModel) -> String? {
        model.menus?.episodes.first(where: \.current)?.number ?? model.staged?.episode
    }
}

/// One episode: its still, number and title, and what the table says of it; revealed, its whole
/// synopsis and buttons.
struct PanelEpisodeRow: View {
    let model: PlayerModel
    let id: AnilistId
    let row: EpisodeRow
    let metrics: PanelMetrics
    let current: Bool
    let metadataOnly: Bool
    /// Spoiler protection hides this row's still and synopsis.
    let hidden: Bool
    let revealed: Bool
    let reveal: () -> Void
    /// After a play: the panel closes.
    let played: () -> Void
    @Environment(AppStore.self) private var store
    @State private var hovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
            if revealed, let more = more {
                more
                    .padding(.top, metrics.reveal)
                    // The row opens on the standard spring; the synopsis fades in 100 ms on.
                    .transition(.asymmetric(
                        insertion: .opacity.animation(.fadeIn.delay(0.1)),
                        removal: .opacity.animation(.quick)))
            }
        }
        .padding(metrics.rowPad)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(fill, in: RoundedRectangle(cornerRadius: metrics.rowRadius, style: .continuous))
        .clipShape(RoundedRectangle(cornerRadius: metrics.rowRadius, style: .continuous))
        .contentShape(.rect)
        .onTapGesture(perform: reveal)
        #if os(macOS)
        .simultaneousGesture(TapGesture(count: 2).onEnded { playAtOnce() })
        .onHover { hovering = $0 }
        #endif
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(current ? [.isButton, .isSelected] : .isButton)
        .accessibilityAction(named: L.actionPlay) { playAtOnce() }
    }

    private var fill: Color {
        if current { return Palette.silk.opacity(0.12) }
        if revealed { return Palette.silk.opacity(0.06) }
        if hovering { return Palette.silk.opacity(0.04) }
        return .clear
    }

    // MARK: The head

    private var head: some View {
        HStack(spacing: metrics.gap) {
            still
            VStack(alignment: .leading, spacing: 2) {
                title
                if let detail = detail, !detail.isEmpty {
                    PanelText(
                        text: detail, size: metrics.detail, line: metrics.detailLine, color: Palette.haze,
                        lines: revealed ? nil : 2)
                        .monospacedDigit()
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var title: some View {
        let words: Text
        if let name = row.title, !name.isEmpty {
            let number = Int(row.number).map { L.episodeNumberShort(number: $0) } ?? row.number
            words = Text("\(Text(number).foregroundStyle(.haze).fontWeight(.medium))\u{2002}\(Text(name))")
        } else {
            words = Text(L.episodeNumberString(number: row.number))
        }
        return words
            .font(.system(size: metrics.title, weight: .semibold))
            .lineSpacing(max(0, metrics.titleLine - metrics.title * 1.2))
            .monospacedDigit()
            .lineLimit(revealed ? nil : 2)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var still: some View {
        let size = metrics.still
        let url = hidden || metadataOnly ? nil : row.thumbnail
        let image = url.flatMap { store.artwork.image($0) }
        let shape = RoundedRectangle(cornerRadius: metrics.stillRadius, style: .continuous)
        return ZStack(alignment: .bottom) {
            shape.fill(Palette.silk.opacity(0.07))
            if let image {
                FilledArt(image: image).transition(.opacity)
            }
            if let progress, progress > 0 {
                GeometryReader { geometry in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Palette.silk.opacity(0.32))
                        Capsule().fill(Palette.silk).frame(width: geometry.size.width * min(1, progress))
                    }
                }
                .frame(height: 3)
                .padding(4)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipShape(shape)
        .opacity(row.completed && !current && !metadataOnly ? 0.62 : 1)
        .animation(.fadeIn, value: image != nil)
        .task(id: url) {
            if let url { await store.artwork.show(url, maxPixelSize: 320) }
        }
        .accessibilityHidden(true)
    }

    /// The silk line on the still: the playhead on the playing row, the history's on a started one.
    private var progress: Double? {
        if metadataOnly { return nil }
        if current {
            guard let duration = model.duration ?? row.durationSecs.map(Double.init), duration > 0 else { return nil }
            return model.position / duration
        }
        return !row.completed && row.watched > 0 ? row.watched : nil
    }

    // MARK: The detail line

    private var notOut: Bool {
        guard let airs = row.airsAt else { return false }
        return Date(timeIntervalSince1970: TimeInterval(airs)) > .now
    }

    private var detail: String? {
        if current {
            let length = model.duration ?? row.durationSecs.map(Double.init) ?? 0
            let left = max(1, Int(((length - model.position) / 60).rounded()))
            return L.episodeRowPlaying(left: L.timeMinutesLeft(minutes: left))
        }
        var parts: [String] = []
        switch row.kind {
        case .filler: parts.append(L.episodeFiller)
        case .mixed: parts.append(L.episodeRowMixed)
        default: break
        }
        if notOut, let airs = row.airsAt {
            parts.append(L.episodeRowAirs(when: AirTime.words(Date(timeIntervalSince1970: TimeInterval(airs)))))
            if let was = row.delayedFrom {
                parts.append(L.episodeRowDelayed(when: AirTime.words(Date(timeIntervalSince1970: TimeInterval(was)))))
            }
            return parts.joined(separator: " · ")
        }
        if row.unsourced, !metadataOnly {
            parts.append(L.episodeUnsourced)
            return parts.joined(separator: " · ")
        }
        let seconds = row.durationSecs.map(Double.init)
        if row.completed {
            parts.append(L.episodeWatched)
        } else if row.watched > 0, !metadataOnly, let seconds {
            parts.append(L.timeMinutesLeft(minutes: max(1, Int((seconds * (1 - row.watched) / 60).rounded()))))
        } else if let seconds, seconds > 0 {
            parts.append(L.episodeRowLength(minutes: max(1, Int((seconds / 60).rounded()))))
        }
        if let source = row.source, !metadataOnly { parts.append(L.episodeRowFrom(source: source)) }
        return parts.joined(separator: " · ")
    }

    // MARK: Revealed

    /// The whole synopsis and the row's buttons, or nothing when it has neither.
    private var more: AnyView? {
        let synopsis: AnyView?
        if hidden, row.description != nil {
            synopsis = AnyView(
                PanelText(text: L.episodeSpoilerHidden, size: metrics.synopsis, line: metrics.synopsisLine, color: Palette.haze)
                    .italic())
        } else if let text = row.description, !text.isEmpty, !notOut {
            synopsis = AnyView(PanelText(text: text, size: metrics.synopsis, line: metrics.synopsisLine, color: Palette.haze))
        } else {
            synopsis = nil
        }
        let buttons = actions
        guard synopsis != nil || !buttons.isEmpty else { return nil }
        return AnyView(
            VStack(alignment: .leading, spacing: metrics.reveal) {
                synopsis
                if !buttons.isEmpty {
                    HStack(spacing: 8) {
                        ForEach(buttons, id: \.title) { button in
                            PanelButton(title: button.title, kind: button.kind, metrics: metrics, action: button.action)
                        }
                    }
                }
            })
    }

    private struct Action {
        let title: String
        let kind: PanelButton.Kind
        let action: () -> Void
    }

    private var actions: [Action] {
        let markWatched = Action(title: L.episodeMarkWatched, kind: .secondary) { [store, id, row] in
            _ = store.send(.setWatched(id: id, episodes: [row.number], watched: true))
        }
        if metadataOnly { return row.completed ? [] : [markWatched] }
        if notOut { return [] }
        if row.unsourced { return row.completed ? [] : [markWatched] }
        if current {
            return [Action(title: L.episodePlayFromBeginning, kind: .secondary) { [model] in model.startOver() }]
        }
        if row.watched > 0, !row.completed {
            return [
                Action(title: L.actionResume, kind: .primary) { play(fromStart: false) },
                Action(title: L.episodePlayFromBeginning, kind: .secondary) { play(fromStart: true) },
            ]
        }
        var all = [Action(title: L.actionPlay, kind: .primary) { play(fromStart: false) }]
        if !row.completed { all.append(markWatched) }
        return all
    }

    /// Whether the row can be played at all: out, sourced, and not the one playing.
    private var playable: Bool { !metadataOnly && !notOut && !row.unsourced && !current }

    private func playAtOnce() {
        guard playable else { return }
        play(fromStart: false)
    }

    private func play(fromStart: Bool) {
        model.playEpisode(row.number, fromStart: fromStart)
        played()
    }
}
