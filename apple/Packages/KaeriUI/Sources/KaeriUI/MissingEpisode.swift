// A title page whose next episode has aired and no source lists yet: the status in the main
// action's place ("Episode 6 aired 25 min ago. Your sources don't have it yet. Kaeri looks again
// every 15 minutes."), Check again and Play the episode before it, and the amber Play crossfading
// in once a source has it. The Mac, the iPad and the iPhone share it.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension TitleAction {
    /// The missing episode the page shows at `now`, when the next episode has aired and no source
    /// lists it or any later one. A part-watched episode is resumed instead.
    static func missing(_ entry: Entry, rows: [EpisodeRow]?, now: Date = .now) -> MissingEpisode? {
        guard let rows else { return nil }
        return missingEpisode(
            rows: rows, next: entry.progress?.next ?? 1, resuming: entry.resume != nil, now: MissingWords.unix(now)
        )
    }
}

/// The status's words: a head and a tail, joined by the app so each sentence is translated once.
enum MissingWords {
    static func unix(_ date: Date) -> Int64 { Int64(date.timeIntervalSince1970.rounded(.down)) }

    /// "Episode 6 aired 25 min ago.", "Episode 6 aired 3 h ago.", "Episode 6 aired on 7 Nov 2026.",
    /// or with no airing time today's words, "No source has this episode".
    static func head(_ missing: MissingEpisode, now: Date) -> String {
        guard let airedAt = missing.airedAt else { return L.episodeUnsourced }
        let episode = Int(missing.episode)
        switch airedAgo(airedAt: airedAt, now: unix(now)) {
        case .minutes(let minutes): return L.episodeMissingAired(episode: episode, ago: L.timeAgo(span: L.timeMinutes(minutes: Int(minutes))))
        case .hours(let hours): return L.episodeMissingAired(episode: episode, ago: L.timeAgo(span: L.timeHours(hours: Int(hours))))
        case .onDate: return L.episodeMissingAiredOn(episode: episode, date: EpisodeFacts.day(Date(unix: airedAt)))
        }
    }

    /// The head as a screen reader says it: "Episode 6 aired 25 minutes ago."
    static func spokenHead(_ missing: MissingEpisode, now: Date) -> String {
        guard let airedAt = missing.airedAt else { return L.episodeUnsourced }
        if case .onDate = airedAgo(airedAt: airedAt, now: unix(now)) { return head(missing, now: now) }
        let span = Spoken.span(max(60, unix(now) - airedAt))
        return L.episodeMissingAired(episode: Int(missing.episode), ago: L.timeAgo(span: span))
    }

    /// What follows the head, by the phase, the last answer to a check asked for, and the network.
    /// Empty with no airing time, where today's words stand alone.
    static func tail(_ missing: MissingEpisode, answer: CheckAnswer?, offline: Bool) -> String {
        switch missing.phase {
        case .undated:
            return ""
        case .longAgo:
            return L.episodeMissingNone
        case .after:
            // Past the quiet window nothing looks by itself, so offline keeps "still".
            return answer == .noAnswer ? L.episodeMissingNoAnswer : L.episodeMissingStill
        case .quiet:
            let first = switch answer {
            case .still: L.episodeMissingStill
            case .noAnswer: L.episodeMissingNoAnswer
            case nil: L.episodeMissingNotYet
            }
            let second = offline ? L.episodeMissingOffline : L.episodeMissingLooksAgain(minutes: Int(missing.everyMinutes))
            return first + " " + second
        }
    }

    /// The hint and tooltip of Check again: during the quiet window only, since it says Kaeri
    /// looks by itself.
    static func checkHint(_ missing: MissingEpisode) -> String? {
        guard missing.phase == .quiet else { return nil }
        return L.episodeCheckAgainHelp(minutes: Int(missing.everyMinutes), hours: Int(missing.forHours))
    }

    /// What a screen reader hears for an answer, once.
    static func spoken(_ kind: CheckSpoken.Kind) -> String {
        switch kind {
        case .checking: L.actionChecking
        case .still: L.episodeMissingStill
        case .noAnswer: L.episodeMissingNoAnswer
        case .found(let episode): L.episodeFoundAnnouncement(episode: Int(episode))
        case .offline: L.stateOfflineLine
        }
    }
}

private extension Date {
    init(unix seconds: Int64) { self.init(timeIntervalSince1970: TimeInterval(seconds)) }
}

/// The sizes of the slot on each device, from the board.
struct MissingMetrics {
    let head: Font
    let tail: Font
    /// Between the head and the tail.
    let lineGap: CGFloat
    /// Between the status and the buttons.
    let statusGap: CGFloat
    let buttonHeight: CGFloat
    let fontSize: CGFloat
    let buttonGap: CGFloat
    /// Whether the buttons share the row's width, as the iPhone's main button spans it.
    let fills: Bool

    static let mac = MissingMetrics(
        head: .mac(13, weight: .semibold), tail: .mac(13), lineGap: 1, statusGap: 10,
        buttonHeight: 34, fontSize: 13, buttonGap: 8, fills: false
    )
    static let pad = MissingMetrics(
        head: .ui(17, weight: .semibold), tail: .ui(15), lineGap: 2, statusGap: 12,
        buttonHeight: 48, fontSize: 17, buttonGap: 12, fills: false
    )
    static let phone = MissingMetrics(
        head: .ui(17, weight: .semibold), tail: .ui(15), lineGap: 2, statusGap: 12,
        buttonHeight: 50, fontSize: 17, buttonGap: 10, fills: true
    )
}

/// The main action's row, which says a missing episode in its place: the status over Check again
/// and Play the episode before it. Once a source lists the episode, the amber Play (`found`)
/// crossfades in where the buttons were (reveal) while the status fades and the slot settles to
/// its new size (standard), so the view never goes through empty. With no missing episode, ever,
/// it is the row as it always was: `found`, then `trailing`.
struct MissingSlot<Found: View, Trailing: View>: View {
    let entry: Entry
    let action: TitleAction
    let metrics: MissingMetrics
    /// False where the main action is not this app's to say, as when a service plays the title.
    var allowed = true
    var spacing: CGFloat = 10
    @ViewBuilder let found: () -> Found
    @ViewBuilder let trailing: () -> Trailing

    var body: some View {
        // The time since airing, and the phase, on the minute: no motion, and nothing said.
        TimelineView(.everyMinute) { context in
            MissingSlotBody(
                entry: entry, action: action, metrics: metrics, allowed: allowed, spacing: spacing, now: context.date,
                found: found, trailing: trailing
            )
        }
    }
}

private struct MissingSlotBody<Found: View, Trailing: View>: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let entry: Entry
    let action: TitleAction
    let metrics: MissingMetrics
    let allowed: Bool
    let spacing: CGFloat
    let now: Date
    let found: () -> Found
    let trailing: () -> Trailing

    /// The missing episode last shown, kept while the amber Play crossfades in over it.
    @State private var kept: MissingEpisode?
    @State private var keptFor: AnilistId?
    /// The crossfade, 0 the status and 1 the Play (reveal).
    @State private var lift = 0.0
    /// The slot's size between the two (standard).
    @State private var settle = 0.0
    /// Raised at each find, so the end of an older crossfade lets a newer one be.
    @State private var finds = 0
    /// The episode before, kept while a check runs: the title's table comes back during it, and
    /// until its sources answer again that episode reads as unsourced.
    @State private var heldPrevious: (episode: UInt32, previous: UInt32)?
    @FocusState private var focus: SlotFocus?
    @AccessibilityFocusState private var spokenFocus: SlotFocus?

    enum SlotFocus: Hashable { case check, previous, play }

    private var missing: MissingEpisode? {
        allowed && action.unsourced && action.handedOff == nil ? TitleAction.missing(entry, rows: store.titles.episodes[entry.id], now: now) : nil
    }

    var body: some View {
        let missing = missing
        let shown = missing ?? (keptFor == entry.id ? kept : nil)
        // While it is missing the status is whole; the crossfade runs only once it is found.
        let lift = missing != nil ? 0 : self.lift
        let settle = missing != nil ? 0 : self.settle
        VStack(alignment: .leading, spacing: 0) {
            if let shown {
                CollapseLayout(shown: 1 - settle) {
                    MissingStatus(missing: shown, answer: answer(shown), offline: store.offline, metrics: metrics, now: now)
                        .padding(.bottom, metrics.statusGap)
                }
                .clipped()
                .opacity(1 - lift)
                .accessibilityHidden(missing == nil)
            }
            HStack(spacing: spacing) {
                SettleLayout(settle: shown == nil ? 1 : settle) {
                    VStack(alignment: .leading, spacing: 16) { found() }
                        .opacity(shown == nil ? 1 : lift)
                        .allowsHitTesting(missing == nil)
                        .accessibilityHidden(missing != nil)
                        .focused($focus, equals: .play)
                        .accessibilityFocused($spokenFocus, equals: .play)
                    if let shown {
                        buttons(shown)
                            .opacity(1 - lift)
                            .allowsHitTesting(missing != nil)
                            .accessibilityHidden(missing == nil)
                    }
                }
                // The wider of the two is cut at the slot's edge while it settles.
                .clipShape(Rectangle().inset(by: shown != nil && missing == nil ? 0 : -40))
                trailing()
            }
        }
        .onChange(of: MissingKey(id: entry.id, missing: missing), initial: true) { old, new in
            changed(from: old, to: new)
            // ⌘R checks again while the page offers Check again.
            let here = new.missing.flatMap { $0.phase == .longAgo ? nil : MissingHere(id: new.id, episode: $0.episode) }
            if here != nil || navigator.checkAgainHere?.id == new.id || old.id != new.id {
                navigator.checkAgainHere = here
            }
        }
        .onDisappear {
            if navigator.checkAgainHere?.id == entry.id { navigator.checkAgainHere = nil }
        }
        .onChange(of: store.missing.spoken) { _, spoken in
            guard let spoken, spoken.id == entry.id else { return }
            // Polite: said after what is being read, once.
            var words = AttributedString(MissingWords.spoken(spoken.kind))
            words.accessibilitySpeechAnnouncementPriority = .default
            AccessibilityNotification.Announcement(words).post()
        }
    }

    private func answer(_ missing: MissingEpisode) -> CheckAnswer? {
        store.missing.answer(entry.id, episode: missing.episode)
    }

    @ViewBuilder
    private func buttons(_ missing: MissingEpisode) -> some View {
        HStack(spacing: metrics.buttonGap) {
            if missing.phase != .longAgo {
                CheckAgainButton(id: entry.id, missing: missing, metrics: metrics)
                    .focused($focus, equals: .check)
                    .accessibilityFocused($spokenFocus, equals: .check)
            }
            if let previous = missing.previous ?? held(missing) {
                Button {
                    store.send(.play(id: entry.id, episode: String(previous)))
                } label: {
                    HStack(spacing: metrics.fills ? 8 : 6) {
                        Image(systemName: "play.fill").font(.board(metrics.fontSize - 4))
                        Text(L.titleButtonPlay(episode: Int(previous)))
                            .lineLimit(1)
                            .minimumScaleFactor(metrics.fills ? 0.7 : 1)
                    }
                    .frame(maxWidth: metrics.fills ? .infinity : nil)
                }
                .buttonStyle(SecondaryCapsuleStyle(height: metrics.buttonHeight, fontSize: metrics.fontSize))
                .focused($focus, equals: .previous)
                .accessibilityFocused($spokenFocus, equals: .previous)
            }
        }
    }

    private func held(_ missing: MissingEpisode) -> UInt32? {
        guard let heldPrevious, heldPrevious.episode == missing.episode, store.missing.isRunning(entry.id) else {
            return nil
        }
        return heldPrevious.previous
    }

    private func changed(from old: MissingKey, to new: MissingKey) {
        if old.id != new.id { heldPrevious = nil }
        if let episode = new.missing, let previous = episode.previous {
            heldPrevious = (episode.episode, previous)
        }
        if let episode = new.missing {
            kept = episode
            keptFor = new.id
            lift = 0
            settle = 0
            return
        }
        guard old.id == new.id, old.missing != nil, kept != nil else {
            // Another title, or nothing was missing: no crossfade.
            kept = nil
            return
        }
        // Found: the Play crossfades in, and the slot settles to its size.
        finds += 1
        let find = finds
        withAnimation(.reveal) { lift = 1 }
        // With Reduce Motion the slot takes its new size at once, under the crossfade.
        withAnimation(reduceMotion ? nil : .standard) { settle = 1 }
        // Once both are done (0.3 s each), the status and the buttons go.
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(350))
            guard finds == find else { return }
            kept = nil
        }
        // Focus that sat on Check again, or the episode before, moves to the Play.
        if focus == .check || focus == .previous { focus = .play }
        if spokenFocus == .check || spokenFocus == .previous { spokenFocus = .play }
    }
}

/// What the slot follows: the title, and its missing episode.
private struct MissingKey: Equatable {
    let id: AnilistId
    let missing: MissingEpisode?
}

/// The status: the water dot and the head in silk, the tail in haze under it. One element for a
/// screen reader. The water dot turns haze a week after airing.
struct MissingStatus: View {
    let missing: MissingEpisode
    let answer: CheckAnswer?
    let offline: Bool
    let metrics: MissingMetrics
    let now: Date

    var body: some View {
        let head = MissingWords.head(missing, now: now)
        let tail = MissingWords.tail(missing, answer: answer, offline: offline)
        let undated = missing.phase == .undated
        VStack(alignment: .leading, spacing: metrics.lineGap) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Circle()
                    .fill(missing.phase == .longAgo ? Palette.haze : Palette.water)
                    .frame(width: 6, height: 6)
                    // Centred on the head's capitals, as the line's middle is on the board.
                    .alignmentGuide(.firstTextBaseline) { $0[.bottom] + metrics.fontSize * 0.35 - 3 }
                // With no airing time, today's words stand where the status goes, in haze.
                Text(head)
                    .font(undated ? metrics.tail : metrics.head)
                    .foregroundStyle(undated ? Palette.haze : Palette.silk)
            }
            if !tail.isEmpty {
                Text(tail)
                    .font(metrics.tail)
                    .foregroundStyle(.haze)
                    .padding(.leading, 14)
                    .contentTransition(.opacity)
                    .animation(.reveal, value: tail)
            }
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: metrics.fills ? .infinity : 560, alignment: .leading)
        .accessibilityElement(children: .ignore)
        // The sentences as they stand, the time in whole words.
        .accessibilityLabel([MissingWords.spokenHead(missing, now: now), tail].filter { !$0.isEmpty }.joined(separator: " "))
    }
}

/// Check again, which says Checking once a check has run 600 ms, and keeps its width.
struct CheckAgainButton: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    let missing: MissingEpisode
    let metrics: MissingMetrics

    var body: some View {
        let checking = store.missing.showsChecking(id)
        let hint = MissingWords.checkHint(missing)
        Button {
            store.missing.checkAgain(id, episode: missing.episode)
        } label: {
            // Both labels are laid out, so the wider sets the button's width either way.
            ZStack {
                HStack(spacing: metrics.fills ? 8 : 6) {
                    Image(systemName: "arrow.clockwise").font(.board(metrics.fontSize - 2, weight: .semibold))
                    Text(L.actionCheckAgain)
                }
                .opacity(checking ? 0 : 1)
                HStack(spacing: metrics.fills ? 8 : 6) {
                    ProgressView()
                        .controlSize(.small)
                        .tint(.silk)
                        #if os(macOS)
                        .scaleEffect(0.8)
                        #endif
                        .frame(width: metrics.fontSize, height: metrics.fontSize)
                    Text(L.actionChecking)
                }
                .opacity(checking ? 1 : 0)
            }
            .lineLimit(1)
            .animation(checking ? .fadeIn : .reveal, value: checking)
            .frame(maxWidth: metrics.fills ? .infinity : nil)
        }
        .buttonStyle(SecondaryCapsuleStyle(height: metrics.buttonHeight, fontSize: metrics.fontSize))
        .accessibilityLabel(checking ? L.actionChecking : L.actionCheckAgain)
        .accessibilityHint(hint ?? "")
        .modifier(OptionalTooltip(text: hint))
    }
}

private struct OptionalTooltip: ViewModifier {
    let text: String?

    func body(content: Content) -> some View {
        if let text { content.tooltip(text) } else { content }
    }
}

/// One view at its natural height times `shown`, its foot kept at the foot: what goes as the
/// status fades out above the Play.
struct CollapseLayout: Layout {
    var shown: Double

    var animatableData: Double {
        get { shown }
        set { shown = newValue }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let view = subviews.first else { return .zero }
        let size = view.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil))
        return CGSize(width: size.width, height: size.height * max(0, min(1, shown)))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        subviews.first?.place(
            at: CGPoint(x: bounds.minX, y: bounds.maxY), anchor: .bottomLeading,
            proposal: ProposedViewSize(width: bounds.width, height: nil)
        )
    }
}

/// Two views on one spot, the first the one that stays: its size goes from the second's to the
/// first's as `settle` goes from 0 to 1. With one view it is that view's size.
struct SettleLayout: Layout {
    var settle: Double

    var animatableData: Double {
        get { settle }
        set { settle = newValue }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let sizes = subviews.map { $0.sizeThatFits(ProposedViewSize(width: proposal.width, height: nil)) }
        guard sizes.count > 1 else { return sizes.first ?? .zero }
        let t = max(0, min(1, settle))
        return CGSize(
            width: sizes[1].width + (sizes[0].width - sizes[1].width) * t,
            height: sizes[1].height + (sizes[0].height - sizes[1].height) * t
        )
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for view in subviews {
            view.place(
                at: CGPoint(x: bounds.minX, y: bounds.maxY), anchor: .bottomLeading,
                proposal: ProposedViewSize(width: proposal.width ?? bounds.width, height: nil)
            )
        }
    }
}
