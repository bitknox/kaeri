// An episode's row on a title page: its still, title, facts line and up to three lines of synopsis,
// on the Mac, the iPad and the iPhone, by the sizes in EpisodeMetrics. The row's height is fixed
// by its kind, never by whether its text has come, so text that arrives later fades in to room
// the row already holds.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The words of one episode row: its still, its title, its facts and its synopsis.
struct EpisodeRowLabel: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var typeSize
    @ScaledMetric(relativeTo: .subheadline) private var textScale = 1.0
    let entry: Entry
    let row: EpisodeRow
    let metrics: EpisodeMetrics
    /// Whether the table keeps room for text.
    let room: Bool
    /// Whether spoiler protection hides this row's still and synopsis.
    let hidden: Bool
    let current: Bool
    /// The number in a column of its own, as on the Mac and the iPad; else it leads the facts.
    let numberColumn: Bool
    /// Whether the viewer asked for this row: its text then comes in once the row has its room.
    var asked = false
    /// The row opens a service outside the app, which its trailing glyph says.
    var external = false

    private var stacked: Bool { typeSize.isAccessibilitySize && metrics.text == .under }

    var body: some View {
        let facts = EpisodeFacts.line(
            row, current: current, minutesLeft: current ? EpisodeFacts.minutesLeft(entry) : nil,
            leadsWithNumber: !numberColumn
        )
        let text = room && !hidden ? row.description : nil
        if stacked {
            stack(facts: facts, text: text)
        } else if metrics.text == .beside {
            beside(facts: facts, text: text)
        } else {
            under(facts: facts, text: text)
        }
    }

    // MARK: Layouts

    private func beside(facts: String, text: String?) -> some View {
        HStack(spacing: 14) {
            if numberColumn { number }
            still
            VStack(alignment: .leading, spacing: 4) {
                title(lines: 1)
                factsLine(facts, lines: 1)
                // About 58 characters a line, however wide the window.
                synopsis(text, lines: 3)
                    .frame(maxWidth: Self.measure, alignment: .leading)
            }
            .frame(maxWidth: .infinity, maxHeight: metrics.still.height, alignment: .topLeading)
            trailing
        }
        .padding(.horizontal, 8)
        .frame(minHeight: metrics.height(room: room, protected: hidden))
    }

    private func under(facts: String, text: String?) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .top, spacing: 12) {
                if numberColumn { number }
                still
                VStack(alignment: .leading, spacing: 4) {
                    title(lines: 2)
                    factsLine(facts, lines: 2)
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)
                if external { trailing }
            }
            if room && !hidden { synopsis(text, lines: 3) }
        }
        .padding(.top, 8)
        .padding(.horizontal, numberColumn ? 8 : 0)
        .frame(minHeight: height, alignment: .top)
        .animation(reduceMotion ? nil : .standard, value: height)
    }

    /// The row at the biggest text sizes: the still across the row, the title on two lines, the
    /// facts, then four lines of text.
    private func stack(facts: String, text: String?) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            still(width: nil)
            title(lines: 2)
            factsLine(facts, lines: 3)
            if room && !hidden { synopsis(text, lines: 4) }
        }
        .padding(.vertical, 8)
        .frame(maxWidth: .infinity, alignment: .topLeading)
    }

    /// The row's height, its text part following the text size.
    private var height: CGFloat {
        let tall = metrics.height(room: room, protected: hidden)
        let base = metrics.short
        return base + (tall - base) * textScale
    }

    // MARK: Parts

    @ViewBuilder
    private var trailing: some View {
        if external {
            Image(systemName: "arrow.up.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                .frame(width: 14)
        } else {
            WatchedCheck(watched: row.completed, size: 11).frame(width: 14)
        }
    }

    private var number: some View {
        Text(row.number)
            .font(Self.numberFont(metrics))
            .monospacedDigit()
            .foregroundStyle(current ? .amber : .haze)
            .animation(.quick, value: current)
            .frame(width: 22, alignment: .trailing)
            .frame(height: metrics.still.height * (metrics.text == .beside ? 1 : 0.2), alignment: .center)
    }

    private var still: some View { still(width: metrics.still.width) }

    private func still(width: CGFloat?) -> some View {
        EpisodeStill(
            entry: entry, row: row, hidden: hidden, unaired: false,
            size: width.map { CGSize(width: $0, height: metrics.still.height) },
            fraction: current ? entry.resume?.fraction : nil
        )
        .opacity(row.completed ? 0.7 : 1)
    }

    private func title(lines: Int) -> some View {
        Text(row.title ?? L.episodeNumberString(number: row.number))
            .font(Self.titleFont(metrics))
            .foregroundStyle(row.completed ? .haze : .silk)
            .lineLimit(lines)
            .multilineTextAlignment(.leading)
            .animation(.quick, value: row.completed)
    }

    private func factsLine(_ facts: String, lines: Int) -> some View {
        Text(facts)
            .font(Self.factsFont(metrics))
            .monospacedDigit()
            .foregroundStyle(.haze)
            .lineLimit(lines)
            .multilineTextAlignment(.leading)
    }

    private func synopsis(_ text: String?, lines: Int) -> some View {
        SynopsisText(text: text, lines: lines, font: Self.synopsisFont(metrics), delay: asked && metrics.text == .under ? 0.2 : 0)
    }

    // MARK: Type

    /// The widest a synopsis beside its still sets its lines: about 58 characters of it.
    static var measure: CGFloat {
        #if os(macOS)
        376
        #else
        436
        #endif
    }

    static func numberFont(_ metrics: EpisodeMetrics) -> Font {
        #if os(macOS)
        .mac(13, weight: .regular)
        #else
        .ui(15)
        #endif
    }

    static func titleFont(_ metrics: EpisodeMetrics) -> Font {
        #if os(macOS)
        .mac(14, weight: .semibold)
        #else
        .headline
        #endif
    }

    static func factsFont(_ metrics: EpisodeMetrics) -> Font {
        #if os(macOS)
        .mac(12)
        #else
        .ui(13)
        #endif
    }

    static func synopsisFont(_ metrics: EpisodeMetrics) -> Font {
        #if os(macOS)
        .mac(13)
        #else
        .ui(15)
        #endif
    }
}

/// Up to `lines` lines of an episode's synopsis, in haze, cut at a word. It fades in when it
/// arrives, into the room its row kept, and replacing text crossfades, never through empty. Its
/// language is the text's own.
struct SynopsisText: View {
    let text: String?
    let lines: Int
    let font: Font
    /// How long its arrival waits: a phone row that grew for it lets it have its room first.
    var delay = 0.0

    var body: some View {
        ZStack(alignment: .topLeading) {
            if let text, !text.isEmpty {
                Text(text)
                    .font(font)
                    .lineSpacing(lineSpacing)
                    .foregroundStyle(.haze)
                    .lineLimit(lines)
                    .multilineTextAlignment(.leading)
                    .environment(\.locale, language(of: text))
                    .id(text)
                    .transition(.opacity.animation(.fadeIn.delay(delay)))
            }
        }
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .animation(.fadeIn, value: text)
    }

    /// 13 pt on 18 on the Mac; the text styles carry their own leading elsewhere.
    private var lineSpacing: CGFloat {
        #if os(macOS)
        2
        #else
        0
        #endif
    }

    @MainActor private func language(of text: String) -> Locale {
        EpisodeLanguage.of(text).map(Locale.init(identifier:)) ?? Locale.current
    }
}

/// An episode's still: its art; blurred, with an eye, while spoiler protection hides it, and
/// crossfading to sharp when it is shown; before the episode airs, the show's banner crop. A row
/// with no still shows the show's own wide art in its place, never blurred, since a show's banner
/// gives nothing away, and a still that comes later fades in over it.
struct EpisodeStill: View {
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let row: EpisodeRow
    let hidden: Bool
    let unaired: Bool
    /// `nil` across the row, at 16:9.
    var size: CGSize? = CGSize(width: 64, height: 36)
    /// How far through the episode the viewer is, as a track along the still's foot.
    var fraction: Double?
    /// Whether this still has been hidden on screen: its blurred copy then stays under the sharp
    /// one, which a reveal fades in over it and a hide fades out from over it.
    @State private var wasHidden = false

    var body: some View {
        Group {
            if let size {
                art(size)
                    .frame(width: size.width, height: size.height)
            } else {
                art(CGSize(width: 320, height: 180))
                    .aspectRatio(16 / 9, contentMode: .fit)
                    .frame(maxWidth: .infinity)
            }
        }
        .clipShape(.rect(cornerRadius: 6))
        .overlay(alignment: .bottom) {
            if let fraction, fraction > 0, !unaired {
                Held(value: fraction) { Meter(fraction: $0, height: 3, silk: true) }
                    .padding(6)
            }
        }
        .onChange(of: hidden, initial: true) { if hidden { wasHidden = true } }
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private func art(_ size: CGSize) -> some View {
        let pixels = Int(max(size.width, 160) * scale)
        ZStack {
            if unaired || row.thumbnail == nil {
                // The show's banner, which cannot spoil an episode.
                WideFallback(entry: entry)
            } else {
                Rectangle().fill(CoverPlate.muted(entry.plateColor))
            }
            if !unaired, let still = row.thumbnail {
                // While hidden only the blurred still is drawn: nothing sharp exists to show
                // before the blur has loaded. Shown, the sharp still fades in over the blurred
                // one on the reveal token, never through the plate.
                if hidden || wasHidden {
                    ArtworkView(url: still, maxPixelSize: 128, flat: .clear)
                        .blur(radius: max(8, size.width / 16))
                        .scaleEffect(1.15)
                }
                if !hidden {
                    ArtworkView(url: still, maxPixelSize: pixels, flat: .clear)
                        .transition(.opacity.animation(.reveal))
                }
                Image(systemName: "eye.slash")
                    .font(.ui(15, weight: .medium))
                    .foregroundStyle(.silk)
                    .opacity(hidden ? 1 : 0)
                    .animation(.quick, value: hidden)
            }
        }
    }
}

/// The show's wide art by the page's own chain, without another episode's still, which would
/// mislabel the row: its banner, else its cover cropped around what draws the eye, else a plate.
struct WideFallback: View {
    let entry: Entry

    var body: some View {
        let art: HeroArt = entry.bannerUrl.map(HeroArt.banner)
            ?? entry.coverUrl.map(HeroArt.cover)
            ?? .waiting
        HeroArtView(art: art, entry: entry)
    }
}

/// The line where spoiler protection starts, over the first row it hides: the eye glyph, why the
/// rows below have no text, and a Show that shows them for the visit. Once shown it says so, with
/// Hide. On a phone Show stands at the line's end; on the Mac and the iPad it follows the words.
struct ProtectionLine: View {
    let episode: Int
    let shown: Bool
    let phone: Bool
    let toggle: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "eye.slash")
                .font(phone ? .ui(13, weight: .medium) : .mac(12, weight: .medium))
                .foregroundStyle(.haze)
                .frame(width: 16)
                .accessibilityHidden(true)
            Text(words)
                .font(phone ? .ui(13) : .mac(12))
                .foregroundStyle(.haze)
                .lineLimit(2)
            if phone { Spacer(minLength: 8) }
            Button(shown ? L.episodeProtectionHide : L.episodeProtectionShow, action: toggle)
                .buttonStyle(.plain)
                .font(phone ? .ui(15, weight: .medium) : .mac(12, weight: .medium))
                .foregroundStyle(.silk)
                .frame(minHeight: phone ? 44 : nil)
                .contentShape(.rect)
                .pointerHover()
            if !phone { Spacer(minLength: 0) }
        }
        .contentTransition(.opacity)
        .animation(.standard, value: shown)
        .padding(.leading, phone ? 0 : 44)
        .frame(minHeight: phone ? 44 : 30)
    }

    private var words: String {
        switch (shown, phone) {
        case (false, false): L.episodeProtectionLine(episode: episode)
        case (false, true): L.episodeProtectionLineShort(episode: episode)
        case (true, false): L.episodeProtectionShown(episode: episode)
        case (true, true): L.episodeProtectionShownShort(episode: episode)
        }
    }
}

#if os(iOS)
/// The preview a long press on an episode lifts above its menu: the still at 16:9, the title, the
/// facts and the synopsis in up to twelve lines, which is where a cut text is read whole. A
/// protected episode's still stays blurred and its text out until Show synopsis is chosen.
struct EpisodePreview: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let row: EpisodeRow

    var body: some View {
        let next = entry.progress?.next ?? 1
        let hidden = EpisodeProtection.hidden(
            row, spoilers: store.spoilerProtection(for: entry.id),
            revealed: navigator.synopsisShown[entry.id] ?? [], all: navigator.synopsisAllShown.contains(entry.id)
        )
        let current = UInt32(row.number) == next
        VStack(alignment: .leading, spacing: 8) {
            EpisodeStill(entry: entry, row: row, hidden: hidden, unaired: false, size: nil)
            Text(row.title ?? L.episodeNumberString(number: row.number))
                .font(.headline)
                .foregroundStyle(.silk)
                .lineLimit(3)
            Text(EpisodeFacts.line(
                row, current: current, minutesLeft: current ? EpisodeFacts.minutesLeft(entry) : nil,
                leadsWithNumber: true
            ))
            .font(.ui(13))
            .foregroundStyle(.haze)
            // A protected episode shows its facts and its blurred still, and no words about what
            // is hidden.
            if !hidden, let text = row.description, !text.isEmpty {
                SynopsisText(text: text, lines: 12, font: .ui(15))
            }
        }
        .padding(16)
        .frame(width: 320, alignment: .topLeading)
        .background(Palette.ground)
    }
}
#endif

/// The control that picks one of a long series' ranges of 100, beside the dub line: the range shown
/// by its first and last episode, and one item per range, each saying Watched or Not out yet where
/// that is all of it. A native pop-up button on the Mac and the iPad, whose View menu also steps
/// through the ranges (Previous Episodes and Next Episodes); a menu in a capsule on a phone.
struct EpisodeRangeMenu: View {
    let plan: EpisodePlan
    let rows: [EpisodeRow]
    let phone: Bool
    let pick: (Int) -> Void

    var body: some View {
        if phone { capsule } else { popUp }
    }

    private var popUp: some View {
        Picker(L.episodeRangeLabel, selection: Binding(get: { plan.index }, set: { pick($0) })) {
            ForEach(plan.ranges.indices, id: \.self) { index in
                Text(EpisodePlan.item(plan.ranges[index], in: rows)).monospacedDigit().tag(index)
            }
        }
        .pickerStyle(.menu)
        .labelsHidden()
        .fixedSize()
        .font(.mac(12))
        .accessibilityValue(EpisodePlan.title(plan.ranges[plan.index], in: rows))
    }

    private var capsule: some View {
        let current = plan.ranges[plan.index]
        return Menu {
            ForEach(plan.ranges.indices, id: \.self) { index in
                Toggle(
                    EpisodePlan.item(plan.ranges[index], in: rows),
                    isOn: Binding(get: { index == plan.index }, set: { _ in pick(index) })
                )
            }
        } label: {
            HStack(spacing: 6) {
                Text(EpisodePlan.title(current, in: rows)).monospacedDigit()
                Image(systemName: "chevron.up.chevron.down")
                    .font(.ui(12, weight: .semibold))
            }
            .font(.ui(15))
            .foregroundStyle(.silk)
            .padding(.horizontal, 12)
            .padding(.vertical, 4)
            .frame(minHeight: 32)
            .overlay(Capsule().strokeBorder(.faintRule))
            .contentShape(.capsule)
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityLabel(L.episodeRangeLabel)
        .accessibilityValue(EpisodePlan.title(current, in: rows))
    }
}
