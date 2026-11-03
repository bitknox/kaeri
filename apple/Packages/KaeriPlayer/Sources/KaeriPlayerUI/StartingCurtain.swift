// The starting screen: from the playback staged to the first frame, the title's art under a calm
// ground, the show and episode, Kaeri's line, one step line that always names what is happening,
// a detail line that fills in when a start is not instant, and the tools to act as the wait
// grows. When the playback never played it says why and what to do next.

import KaeriDesign
import KaeriKit
import KaeriPlayer
import KaeriStore
import KaeriStrings
import SwiftUI

/// The art at the depth its brightness needs, the column over it. Between episodes the last
/// frame fades through the ground colour, never black.
struct StartingCurtain: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let model: PlayerModel
    let metrics: PlayerMetrics
    /// How deep the ground sits over each piece of art, measured once when it has loaded.
    @State private var depths: [String: Double] = [:]
    /// The heights of the column and of what opens under it, for sliding the column up by any
    /// overflow.
    @State private var columnHeight: CGFloat = 0
    /// The column's height while it is a start, with the room of every button that can arrive
    /// kept: what it is centred on, so nothing in it moves as lines and buttons come, or when it
    /// turns into a failure.
    @State private var reservedHeight: CGFloat = 0
    @State private var belowHeight: CGFloat = 0
    @State private var listHeight: CGFloat = 0

    var body: some View {
        // The title the engine names with the starting screen, else the one staged: its art by
        // the hero-art rule where the title is known here, else the banner the engine sends.
        let id = model.starting?.id ?? model.staged?.id
        let entry = id.flatMap { store.titles[$0] }
        let url = entry.map { store.heroArt($0).url } ?? model.starting?.bannerUrl
        let image = url.flatMap { store.artwork.image($0) }
        // The depth is known before the art fades in, so it never changes on screen.
        let ground = url.flatMap { depths[$0] }
        GeometryReader { proxy in
            ZStack {
                Palette.ground
                if let image, let ground {
                    FilledArt(image: image)
                        .transition(.opacity)
                    Palette.ground.opacity(ground)
                }
                column(in: proxy.size)
            }
        }
        .animation(.fadeIn, value: image != nil && ground != nil)
        .clipped()
        .task(id: url) {
            // As sharp as the screen, which the player may come to fill.
            if let url { await store.artwork.show(url, maxPixelSize: ScreenPixels.longSide) }
        }
        .onChange(of: (url ?? "") + (image == nil ? "" : "+"), initial: true) {
            if let url, let image, depths[url] == nil {
                depths[url] = GroundDepth.depth(forLuminance: GroundDepth.centreLuminance(of: image) ?? 0)
            }
        }
        .task(id: entry?.id) {
            // A title with no banner shows a still already seen, which the episode list has.
            if let entry, entry.bannerUrl == nil, store.titles.episodes[entry.id] == nil {
                store.loadEpisodes(entry.id)
            }
        }
        .modifier(StartingAnnouncements(model: model))
    }

    // MARK: The column

    private func column(in size: CGSize) -> some View {
        let width = min(520, max(0, size.width - 2 * (metrics.touch ? 56 : 48)))
        let bottom: CGFloat = metrics.touch ? 40 : 32
        let top: CGFloat = metrics.touch ? 24 : 32
        // Centred on the column as a start keeps it, its top fixed there so a longer failure
        // grows downward only; slid up only by what the list would run past the bottom.
        let height = reservedHeight > 0 ? reservedHeight : columnHeight
        let room = (size.height - height) / 2
        let overflow = max(0, room + height + 16 + belowHeight - (size.height - bottom))
        let lift = min(overflow, max(0, room - top))
        return closed
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                columnHeight = $0
                if !failed { reservedHeight = $0 }
            }
            .overlay(alignment: .bottomLeading) {
                below(cap: size.height - bottom - top - columnHeight - 16)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { belowHeight = $0 }
                    .alignmentGuide(.bottom) { $0[.top] }
            }
            .frame(width: width, alignment: .leading)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .padding(.top, max(0, room - lift))
            .animation(.standard, value: lift)
            .foregroundStyle(.silk)
            .shadow(color: Color(rgb: 0x0A0C18, opacity: 0.5), radius: 1, y: 1)
            .animation(.standard, value: model.failure)
            .accessibilityElement(children: .contain)
    }

    private var failed: Bool { model.failure != nil }

    /// Reduce Motion, or a screenshot run that asks for it.
    private var motionReduced: Bool {
        #if FIXTURES
        if ProcessInfo.processInfo.environment["KAERI_REDUCE_MOTION"] == "1" { return true }
        #endif
        return reduceMotion
    }

    /// Everything that is there from the first frame: the show, the episode, the line and its
    /// room, the step and detail lines and the row, each with its room kept.
    private var closed: some View {
        let walk = model.walk
        let now = model.clock
        let waiting = failed == false && walk?.waiting == true
        let demo = (model.starting?.id ?? model.staged?.id).flatMap { store.titles[$0] }?.isDemo == true
        let episode = model.startingEpisode ?? ""
        return VStack(alignment: .leading, spacing: 0) {
            Text(model.showTitle)
                .font(.showTitle(size: metrics.touch ? 22 : 30))
                .lineLimit(2)
            Text(demo ? L.playerStartingPart(part: episode) : model.episodeLine)
                .font(.system(size: metrics.touch ? 15 : 15))
                .foregroundStyle(.haze)
                .padding(.top, 6)
            if demo {
                // The licence asks for the credit wherever the work shows.
                Text(L.playerDemoCredit)
                    .font(.system(size: 13))
                    .foregroundStyle(.haze)
                    .padding(.top, 4)
            }
            LoaderLine(reduceMotion: motionReduced, progress: waiting ? nil : walk?.progress)
                .frame(height: 2)
                .padding(.top, 14)
                .opacity(failed ? 0 : 1)
                .animation(.standard, value: failed)
                .accessibilityHidden(true)
            lines(walk: walk, now: now, episode: episode)
            StartActions(model: model, metrics: metrics)
                .padding(.top, 16)
        }
    }

    /// The step and detail lines as one element that can be read at any time.
    private func lines(walk: StartingWalk?, now: ContinuousClock.Instant, episode: String) -> some View {
        let words = Self.words(walk: walk, now: now, failure: model.failure, episode: episode)
        return VStack(alignment: .leading, spacing: 0) {
            StepLine(words: words, touch: metrics.touch)
                .padding(.top, 12)
            DetailLine(text: words.detail, touch: metrics.touch, lines: words.failure ? 2 : 1)
                .padding(.top, 3)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(words.detail.isEmpty ? words.step : L.playerStartStatus(step: words.step, detail: words.detail))
        .accessibilityAddTraits(.updatesFrequently)
    }

    /// What the two lines say now.
    struct Words: Equatable {
        var step: String
        var detail: String
        var failure = false
    }

    static func words(
        walk: StartingWalk?, now: ContinuousClock.Instant, failure: PlaybackFailure?, episode: String
    ) -> Words {
        if let failure {
            return Words(step: L.playerFailedTitleText(episode: failure.episode), detail: failure.view.detail, failure: true)
        }
        guard let walk else { return Words(step: L.playerStartingText(episode: episode), detail: "") }
        return Words(step: walk.stepLine(at: now), detail: walk.detail(at: now))
    }

    // MARK: What opens under the row

    @ViewBuilder
    private func below(cap: CGFloat) -> some View {
        Group {
            if model.choosingSource {
                scrolling(SourceList(model: model, metrics: metrics), cap: cap)
            } else if model.startDetailsOpen, !(model.walk?.waiting ?? false), model.walk != nil {
                scrolling(StartDetails(model: model, metrics: metrics), cap: cap)
            }
        }
    }

    /// The list at its height, or scrolling in what the screen has left, as on a phone held wide.
    private func scrolling(_ list: some View, cap: CGFloat) -> some View {
        let room = max(cap, 96)
        return Group {
            if listHeight > room {
                ScrollView { list }.scrollIndicators(.hidden).frame(height: room)
            } else {
                list
            }
        }
        .background {
            // The list at its own height, unseen, so what it needs is known whichever way it shows.
            list.fixedSize(horizontal: false, vertical: true)
                .hidden()
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { listHeight = $0 }
        }
        .padding(.top, 16)
        .transition(.opacity)
    }
}

// MARK: The lines

/// The step line, held in its room: a new text crossfades in place, and digits roll.
private struct StepLine: View {
    let words: StartingCurtain.Words
    let touch: Bool

    var body: some View {
        let font = Font.system(size: touch ? 17 : 15, weight: words.failure ? .semibold : .medium)
        ZStack(alignment: .topLeading) {
            // Its room, kept at the current text size.
            Text(" ").font(font).hidden()
            Group {
                if words.failure {
                    HStack(spacing: 8) {
                        Circle().fill(Palette.vermilion).frame(width: 8, height: 8)
                        Text(words.step)
                    }
                } else {
                    Text(words.step).contentTransition(.numericText())
                }
            }
            .font(font)
            .id(Self.identity(words))
            .transition(.opacity)
        }
        .animation(.standard, value: Self.identity(words))
        .animation(.standard, value: words.step)
    }

    /// Words change in place by crossfading; a number alone rolls.
    static func identity(_ words: StartingCurtain.Words) -> String {
        (words.failure ? "failed:" : "") + words.step.filter { !$0.isNumber }
    }
}

/// The detail line, 13 pt haze with figures that keep their width, its room kept.
private struct DetailLine: View {
    let text: String
    let touch: Bool
    /// One line during a start, with the end cut off; a failure's reason and who was asked may take two.
    let lines: Int

    var body: some View {
        let font = Font.system(size: touch ? 15 : 13)
        ZStack(alignment: .topLeading) {
            Text(" ").font(font).hidden()
            Text(text)
                .font(font)
                .monospacedDigit()
                .foregroundStyle(.haze)
                .lineLimit(lines)
                .truncationMode(.tail)
                .fixedSize(horizontal: false, vertical: true)
                .contentTransition(.numericText())
                .id(text.filter { !$0.isNumber })
                .transition(.opacity)
        }
        .animation(.fadeIn, value: text.filter { !$0.isNumber })
        .animation(.standard, value: text)
    }
}

// MARK: The row

/// The row of buttons, in arrival order with Cancel first, so a new button lands at the
/// trailing end and nothing already there moves. Its room is kept from the start.
private struct StartActions: View {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    let metrics: PlayerMetrics

    private struct Item: Identifiable {
        var id: String
        var label: String
        var kind: PillButtonStyle.Kind
        var chevron: Bool? = nil
        var shortcut: KeyboardShortcut? = nil
        /// A button that no longer applies keeps its place, unseen and unreachable.
        var spent = false
        /// Skip's label is cut to this width, so the room kept for it is known.
        var width: CGFloat? = nil
        /// The most a label may take, cut past it.
        var cap: CGFloat? = nil
        var action: () -> Void = {}
    }

    /// The widest a Skip button gets, and the room kept for it.
    private static let skipWidth: CGFloat = 160

    var body: some View {
        let items = self.items
        // The row's room is the tallest of the rows it can be: every button a start can bring,
        // and a failure's, laid out unseen under the real one at the same width.
        ZStack(alignment: .topLeading) {
            row(reserve(forStart: true)).hidden().accessibilityHidden(true).allowsHitTesting(false)
            row(reserve(forStart: false)).hidden().accessibilityHidden(true).allowsHitTesting(false)
            row(items)
        }
        .frame(maxWidth: .infinity, minHeight: metrics.button, alignment: .topLeading)
        .animation(.fadeIn, value: items.map(\.id))
        .animation(.standard, value: items.map(\.spent))
        .animation(.standard, value: model.startDetailsOpen)
    }

    private func row(_ items: [Item]) -> some View {
        WrapRow(spacing: 8, lineSpacing: 8) {
            ForEach(items) { item in
                Button {
                    item.action()
                } label: {
                    HStack(spacing: 5) {
                        Text(item.label)
                            .lineLimit(1)
                            .truncationMode(.tail)
                            .frame(width: item.width, alignment: .leading)
                            .frame(maxWidth: item.cap, alignment: .leading)
                        if let up = item.chevron {
                            Image(systemName: "chevron.down")
                                .font(.system(size: metrics.touch ? 11 : 9, weight: .semibold))
                                .rotationEffect(.degrees(up ? 180 : 0))
                        }
                    }
                }
                .buttonStyle(PillButtonStyle(kind: item.kind, metrics: metrics))
                .modifier(Shortcut(shortcut: item.spent ? nil : item.shortcut))
                .opacity(item.spent ? 0 : 1)
                .disabled(item.spent)
                .accessibilityHidden(item.spent)
                .transition(.opacity)
            }
        }
    }

    /// Every button the start (or the failure) can show, for its room alone.
    private func reserve(forStart start: Bool) -> [Item] {
        guard start else {
            return [
                Item(id: "choose", label: L.playerChooseAnotherSource, kind: .primary),
                Item(id: "again", label: L.actionTryAgain, kind: .secondary),
                Item(id: "details", label: L.playerStartDetails, kind: .quiet, chevron: false),
                Item(id: "close", label: L.actionClose, kind: .quiet),
            ]
        }
        var items = [Item(id: "cancel", label: L.actionCancel, kind: .quiet)]
        if model.walk?.resume != nil {
            items.append(Item(id: "begin", label: L.playerStartFromBeginning, kind: .secondary))
        }
        items += [
            Item(id: "choose", label: L.playerChooseSource, kind: .secondary),
            Item(id: "details", label: L.playerStartDetails, kind: .quiet, chevron: false),
            Item(id: "skip", label: "", kind: .secondary, width: Self.skipWidth),
        ]
        return items
    }

    private struct Shortcut: ViewModifier {
        let shortcut: KeyboardShortcut?
        func body(content: Content) -> some View {
            if let shortcut { content.keyboardShortcut(shortcut) } else { content }
        }
    }

    private var items: [Item] {
        if let failure = model.failure { return failureItems(failure) }
        return (model.walk?.actions(at: model.clock) ?? [.cancel]).map(item)
    }

    private func item(_ action: StartAction) -> Item {
        let cancel: KeyboardShortcut? = model.choosingSource ? nil : .cancelAction
        switch action {
        case .cancel:
            return Item(id: "cancel", label: L.actionCancel, kind: .quiet, shortcut: cancel) { model.viewerClosed() }
        case .playFromBeginning:
            return Item(id: "begin", label: L.playerStartFromBeginning, kind: .secondary, spent: model.walk?.fromBeginning == true) {
                model.playFromBeginning()
            }
        case .chooseSource:
            return Item(id: "choose", label: L.playerChooseSource, kind: .secondary) {
                model.startDetailsOpen = false
                model.chooseSource()
            }
        case .details:
            return Item(id: "details", label: L.playerStartDetails, kind: .quiet, chevron: model.startDetailsOpen) {
                model.cancelChoosingSource()
                model.startDetailsOpen.toggle()
            }
        case .skip(let label):
            return Item(id: "skip", label: label, kind: .secondary, cap: Self.skipWidth) { model.skipSource() }
        case .showDownloads:
            return Item(id: "downloads", label: L.stateOfflineShowDownloads, kind: .secondary) { model.openDownloads?() }
        }
    }

    /// A failure's row, the primary first: what to do next, never a dead end.
    private func failureItems(_ failure: PlaybackFailure) -> [Item] {
        let choose = Item(id: "choose", label: L.playerChooseAnotherSource, kind: .secondary) { model.chooseSource() }
        let again = Item(id: "again", label: L.actionTryAgain, kind: .secondary, shortcut: .init("r")) { model.tryAgain() }
        let details = Item(id: "details", label: L.playerStartDetails, kind: .quiet, chevron: model.startDetailsOpen) {
            model.cancelChoosingSource()
            model.startDetailsOpen.toggle()
        }
        let close = Item(id: "close", label: L.actionClose, kind: .quiet, shortcut: .cancelAction) { model.viewerClosed() }
        func primary(_ item: Item) -> Item { var item = item; item.kind = .primary; item.shortcut = .defaultAction; return item }
        switch failure.view.lead {
        case .chooseSource: return [primary(choose), again, details, close]
        case .tryAgain: return [primary(again), choose, details, close]
        case .fixMatch:
            let fix = Item(id: "fix", label: L.playerStartFixMatch, kind: .primary, shortcut: .defaultAction) {
                if let id = failure.id { model.openFixMatch?(id) }
            }
            return [fix, choose, details, close]
        case .offline:
            let downloads = Item(id: "downloads", label: L.stateOfflineShowDownloads, kind: .secondary) { model.openDownloads?() }
            return [primary(again), downloads, close]
        case .addStore:
            let add = Item(id: "store", label: L.playerAddPluginStore, kind: .primary, shortcut: .defaultAction) {
                model.openPluginStores?()
            }
            return [add, close]
        case .close:
            var only = close
            only.kind = .secondary
            return [only]
        }
    }
}

/// Views in a row that wraps to a second line when the text is large.
private struct WrapRow: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = arrange(width: proposal.width ?? .infinity, subviews: subviews)
        return CGSize(width: proposal.width ?? result.width, height: result.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrange(width: bounds.width, subviews: subviews)
        for (index, frame) in result.frames.enumerated() {
            subviews[index].place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY), proposal: .unspecified)
        }
    }

    private func arrange(width: CGFloat, subviews: Subviews) -> (frames: [CGRect], width: CGFloat, height: CGFloat) {
        var frames: [CGRect] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var used: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width {
                x = 0
                y += rowHeight + lineSpacing
                rowHeight = 0
            }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            used = max(used, x - spacing)
            rowHeight = max(rowHeight, size.height)
        }
        return (frames, used, y + rowHeight)
    }
}

// MARK: Details

/// One row per step: done in haze with a tick, the one under way in silk with its seconds,
/// then the audio and subtitles with the switch.
struct StartDetails: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    private var rowHeight: CGFloat { metrics.touch ? 44 : 26 }
    private var font: Font { .system(size: metrics.touch ? 15 : 13) }

    var body: some View {
        let walk = model.walk
        let now = model.clock
        VStack(alignment: .leading, spacing: 0) {
            ForEach(walk?.detailRows ?? []) { row in
                HStack(spacing: 8) {
                    icon(row.kind).frame(width: 16)
                    Text(row.title)
                        .foregroundStyle(row.kind == .now ? Palette.silk : Palette.haze)
                        .fontWeight(row.kind == .now ? .medium : .regular)
                        .lineLimit(1)
                    Spacer(minLength: 8)
                    if let took = row.took ?? (row.kind == .now ? max(.zero, now - row.began) : nil) {
                        Text(Self.seconds(took)).foregroundStyle(Palette.haze).monospacedDigit()
                    }
                    if row.kind == .now, model.failure == nil, walk?.skipLabel(at: now) != nil {
                        Button(L.actionSkip) { model.skipSource() }
                            .buttonStyle(.plain)
                            .foregroundStyle(.silk)
                            .padding(.horizontal, metrics.touch ? 12 : 6)
                            .frame(height: rowHeight)
                            .padding(.trailing, metrics.touch ? -12 : -6)
                            .contentShape(.rect)
                            .pointingHand()
                    }
                }
                .frame(height: rowHeight)
            }
            if let speed = walk?.speedFact {
                HStack(spacing: 8) {
                    Color.clear.frame(width: 16)
                    Text(speed).foregroundStyle(Palette.haze).monospacedDigit()
                    Spacer(minLength: 0)
                }
                .frame(height: rowHeight)
            }
            Rectangle().fill(Palette.silk.opacity(0.14)).frame(height: 1).padding(.vertical, 6)
            HStack(spacing: 8) {
                Text(L.playerStartTranslation).foregroundStyle(Palette.haze).lineLimit(1)
                Spacer(minLength: 8)
                TranslationSwitch(model: model, metrics: metrics)
            }
            .frame(height: max(rowHeight, (metrics.touch ? 32 : 24) + 12))
        }
        .font(font)
        .padding(.top, 6)
        .foregroundStyle(.silk)
    }

    @ViewBuilder
    private func icon(_ kind: StartRow.Kind) -> some View {
        switch kind {
        case .done: Image(systemName: "checkmark").font(.system(size: 10, weight: .semibold)).foregroundStyle(Palette.haze)
        case .missed, .resting, .fact: Image(systemName: "minus").font(.system(size: 10, weight: .semibold)).foregroundStyle(Palette.haze)
        case .now: Circle().fill(Palette.silk).frame(width: 7, height: 7)
        }
    }

    /// "0.7 s": the shared seconds wording with a tenth, as the Details rows keep it.
    static func seconds(_ duration: Duration) -> String {
        let tenths = (duration.seconds * 10).rounded() / 10
        let number = tenths.formatted(.number.precision(.fractionLength(1)))
        let template = L.playerStartSeconds(seconds: 0)
        guard template.contains("0") else { return L.playerStartSeconds(seconds: Int(tenths)) }
        return template.replacingOccurrences(of: "0", with: number)
    }
}

/// Subtitled and Dubbed: the translation the episode plays in. Switching starts it again in the
/// other one.
private struct TranslationSwitch: View {
    let model: PlayerModel
    let metrics: PlayerMetrics

    var body: some View {
        let current = model.walk?.translation ?? .sub
        HStack(spacing: 0) {
            segment(L.playerStartSubtitled, translation: .sub, current: current)
            segment(L.audioDubbed, translation: .dub, current: current)
        }
        .padding(2)
        .frame(height: metrics.touch ? 32 : 24)
        .background(Capsule().fill(Palette.silk.opacity(0.10)))
        .accessibilityElement(children: .contain)
    }

    private func segment(_ title: String, translation: KaeriKit.Translation, current: KaeriKit.Translation) -> some View {
        Button {
            if translation != current { model.startWith(translation) }
        } label: {
            Text(title)
                .font(.system(size: metrics.touch ? 14 : 12, weight: .medium))
                .padding(.horizontal, metrics.touch ? 14 : 10)
                .frame(maxHeight: .infinity)
                .background(Capsule().fill(Palette.silk.opacity(translation == current ? 0.22 : 0)))
                .animation(.quick, value: current)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(translation == current ? .isSelected : [])
        .pointingHand()
    }
}

// MARK: Said aloud

/// The start for VoiceOver: the screen opening, a slow start once, being offline, and the
/// outcome. Never every step.
private struct StartingAnnouncements: ViewModifier {
    let model: PlayerModel

    func body(content: Content) -> some View {
        content
            .onChange(of: model.staged?.episode, initial: true) {
                guard model.failure == nil, let staged = model.staged else { return }
                announce(L.playerStartAnnounce(episode: staged.episode, title: staged.title))
            }
            .onChange(of: slow) { _, slow in
                if slow { announce(L.playerStartStillStarting) }
            }
            .onChange(of: waiting) { _, waiting in
                if waiting, let episode = model.startingEpisode { announce(L.playerStartOfflineDetail(episode: episode)) }
            }
            .onChange(of: model.failure) { _, failure in
                guard let failure else { return }
                announce(L.playerStartFailedAnnounce(title: L.playerFailedTitleText(episode: failure.episode), reason: failure.view.detail), urgent: true)
            }
    }

    private var slow: Bool {
        model.failure == nil && (model.walk?.isSlow(at: model.clock) ?? false)
    }

    private var waiting: Bool {
        model.failure == nil && (model.walk?.waiting ?? false)
    }

    private func announce(_ words: String, urgent: Bool = false) {
        var text = AttributedString(words)
        if urgent { text.accessibilitySpeechAnnouncementPriority = .high }
        AccessibilityNotification.Announcement(text).post()
    }
}

private extension Duration {
    var seconds: Double { Double(components.seconds) + Double(components.attoseconds) / 1e18 }
}
