// An episode's row on a title page: its still, title, facts line and up to three lines of synopsis (two where it sits beside the still),
// on the Mac, the iPad and the iPhone, by the sizes in EpisodeMetrics. The row's height is set
// by its kind, never by whether its text has come, so text that arrives later fades in to room
// the row already holds. A row that fits (the iPhone's, and the Mac's below its widest list)
// holds that room only while its text may still come: once its details are known it fits what
// it has, resizing in the crossfade they arrive in.

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
    /// The people followed whose progress stops here, in the marks' column before the row's
    /// trailing end; nil when the table has no marks, which leaves the column out.
    var marks: [FollowedMark]? = nil
    /// Whether a cut synopsis says where its More goes, which the row lays over itself.
    var expandable = false
    /// Whether the synopsis shows whole, after its More.
    var expanded = false

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
        // The text is pinned to the still's top, so every title in the list starts at one height.
        HStack(alignment: .top, spacing: 14) {
            if numberColumn { number }
            still
            VStack(alignment: .leading, spacing: 0) {
                title(lines: Self.besideTitleLines)
                factsLine(facts, lines: 1).padding(.top, 2)
                if metrics.holdsText(room: room, protected: hidden, pending: row.synopsisPending, text: text) {
                    // About 58 characters a line, however wide the window.
                    synopsis(
                        text, lines: 2,
                        reserve: metrics.reservesText(room: room, protected: hidden, pending: row.synopsisPending)
                    )
                    .padding(.top, 8)
                    .frame(maxWidth: Self.measure, alignment: .leading)
                    .transition(.opacity)
                }
            }
            // The still sets the row's floor; where the row fits, text that runs past it stands
            // taller, and the row with it.
            .frame(maxWidth: .infinity, maxHeight: metrics.fits || expanded ? nil : metrics.still.height, alignment: .topLeading)
            if let marks { FollowedMarks(marks: marks) }
            trailing
        }
        .padding(.horizontal, 8)
        .padding(.vertical, metrics.besideRoom)
        .frame(minHeight: metrics.height(room: room, protected: hidden))
        .animation(metrics.fits && !reduceMotion ? .fadeIn : nil, value: row.synopsisPending)
    }

    private func under(facts: String, text: String?) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            // The Mac's gaps, so its words start where its other rows' do; the phone's own.
            HStack(alignment: .top, spacing: metrics.ruled ? 12 : 14) {
                if numberColumn { number }
                // A ruled row carries the marks on the still's corner, so the title has the
                // row's whole width.
                still.overlay(alignment: .topTrailing) {
                    if metrics.ruled, let marks { FollowedMarks(marks: marks).padding(6) }
                }
                VStack(alignment: .leading, spacing: 0) {
                    title(lines: 2)
                    factsLine(facts, lines: 2).padding(.top, 2)
                }
                .frame(maxWidth: .infinity, alignment: .topLeading)
                if !metrics.ruled, let marks { FollowedMarks(marks: marks) }
                if external { trailing }
            }
            if metrics.holdsText(room: room, protected: hidden, pending: row.synopsisPending, text: text) {
                synopsis(text, lines: 3).transition(.opacity)
            }
        }
        .padding(.top, metrics.pad)
        .padding(.bottom, metrics.fits ? metrics.pad : 0)
        .padding(.horizontal, numberColumn ? 8 : 0)
        // The rule under a ruled row is the page's, after anything it adds under the row.
        .frame(minHeight: height - (metrics.ruled ? EpisodeMetrics.rule : 0), alignment: .top)
        // A reveal grows the row, then its text fades in; details that arrive resize it in their
        // own fade, never as a glide of its own.
        .animation(reduceMotion ? nil : .standard, value: metrics.height(room: room, protected: hidden))
        .animation(reduceMotion ? nil : .fadeIn, value: row.synopsisPending)
    }

    /// The row at the biggest text sizes: the still across the row, the title on two lines, the
    /// facts, then four lines of text.
    private func stack(facts: String, text: String?) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            still(width: nil).padding(.bottom, 8)
            title(lines: 2)
            factsLine(facts, lines: 3).padding(.top, 2)
            if let marks, !marks.isEmpty {
                PictureStack(people: marks.map { ($0.name, $0.avatarUrl) }, size: MarkMetrics.current.size, ring: Palette.ground)
                    .padding(.top, 6)
            }
            if metrics.holdsText(room: room, protected: hidden, pending: row.synopsisPending, text: text) {
                synopsis(text, lines: 4).padding(.top, 8)
            }
        }
        .padding(.vertical, metrics.pad)
        .frame(maxWidth: .infinity, alignment: .topLeading)
    }

    /// The row's least height, its text part following the text size.
    private var height: CGFloat {
        let least = metrics.least(room: room, protected: hidden, pending: row.synopsisPending)
        let base = metrics.short
        return base + (least - base) * textScale
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
            // Amber marks what plays: an episode no source lists stays haze until one does.
            .foregroundStyle(current && (!row.unsourced || external) ? .amber : .haze)
            .animation(.quick, value: current)
            .animation(.reveal, value: row.unsourced)
            // A number past 99 keeps one line, standing into the row's margin.
            .lineLimit(1)
            .fixedSize()
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
            .foregroundStyle(.silk)
            .lineLimit(lines)
            .multilineTextAlignment(.leading)
    }

    private func factsLine(_ facts: String, lines: Int) -> some View {
        // An aired episode no source lists says how long ago it aired, on the minute, with no
        // motion; once a source lists it the line crossfades to the usual one.
        TimelineView(.everyMinute) { context in
            Text(row.unsourced ? factsLine(at: context.date) : facts)
                .contentTransition(.opacity)
        }
            .font(Self.factsFont(metrics))
            .monospacedDigit()
            .foregroundStyle(.haze)
            .lineLimit(lines)
            .multilineTextAlignment(.leading)
    }

    private func factsLine(at now: Date) -> String {
        EpisodeFacts.line(
            row, current: current, minutesLeft: current ? EpisodeFacts.minutesLeft(entry) : nil,
            leadsWithNumber: !numberColumn, now: now
        )
    }

    private func synopsis(_ text: String?, lines: Int, reserve: Bool = false) -> some View {
        SynopsisText(
            text: text, lines: lines, font: Self.synopsisFont(metrics),
            delay: asked && metrics.text == .under ? 0.2 : 0, reserve: reserve,
            expandable: expandable, expanded: expanded
        )
    }

    // MARK: Type

    /// The title's lines where the text sits beside the still: two on the Mac, which they fit
    /// beside either still with the synopsis's two, one on the iPad.
    static var besideTitleLines: Int {
        #if os(macOS)
        2
        #else
        1
        #endif
    }

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
    /// Whether it keeps its lines open while its text may still come.
    var reserve = false
    /// Whether it says where a More or a Less belongs (`SynopsisMorePlace`), for a row that
    /// lays the control over itself, outside its own button.
    var expandable = false
    /// Whether the whole text shows, uncut, with room under it for Less.
    var expanded = false
    @State private var full: CGFloat = 0
    @State private var shown: CGFloat = 0

    var body: some View {
        ZStack(alignment: .topLeading) {
            if reserve {
                Text(Array(repeating: " ", count: lines).joined(separator: "\n"))
                    .font(font)
                    .lineSpacing(lineSpacing)
                    .lineLimit(lines)
                    .hidden()
            }
            if let text, !text.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text(text)
                        .font(font)
                        .lineSpacing(lineSpacing)
                        .foregroundStyle(.haze)
                        .lineLimit(expanded ? nil : lines)
                        .multilineTextAlignment(.leading)
                        .environment(\.locale, language(of: text))
                        // More and Less crossfade the cut and the whole text, never moving a word.
                        .contentTransition(.opacity)
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { shown = $0 }
                        // The whole text at the same width, measured unseen: more than what shows
                        // means the text is cut.
                        .background(alignment: .topLeading) {
                            if expandable {
                                Text(text)
                                    .font(font)
                                    .lineSpacing(lineSpacing)
                                    .fixedSize(horizontal: false, vertical: true)
                                    .hidden()
                                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { full = $0 }
                            }
                        }
                    // Less's line under the whole text.
                    if expandable && expanded {
                        Text(L.shellLess).font(font.weight(.semibold)).hidden()
                    }
                }
                .anchorPreference(key: SynopsisMorePlace.self, value: .bounds) { bounds in
                    expandable && (expanded || full > shown + 1)
                        ? SynopsisMorePlace.Place(bounds: bounds, expanded: expanded) : nil
                }
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

/// Where a cut synopsis's More goes, at the end of its last line, or an expanded one's Less, on
/// the line under it: the row lays the control over itself from here, so it is a button of its
/// own beside the row's, never inside it.
struct SynopsisMorePlace: PreferenceKey {
    struct Place {
        let bounds: Anchor<CGRect>
        let expanded: Bool
    }

    static let defaultValue: Place? = nil

    static func reduce(value: inout Place?, nextValue: () -> Place?) {
        value = value ?? nextValue()
    }
}

/// The More at the end of a cut synopsis in an episode row, or the Less under an expanded one,
/// as the title page's synopsis has them. `fill` is what lies under the row (the current row's
/// selection), which More's fade over the last line's end carries.
struct SynopsisMoreButton: View {
    let place: SynopsisMorePlace.Place
    let font: Font
    let fill: Color
    var dimmed = false
    let toggle: () -> Void

    var body: some View {
        GeometryReader { proxy in
            let rect = proxy[place.bounds]
            Color.clear
                .frame(width: rect.width, height: rect.height)
                .overlay(alignment: place.expanded ? .bottomLeading : .bottomTrailing) {
                    // Each its own: the one going leaves at once, where it stood, and the one
                    // coming fades in once the row has its new height.
                    if place.expanded {
                        button.transition(.asymmetric(insertion: .opacity.animation(.fadeIn.delay(0.25)), removal: .identity))
                    } else {
                        button.transition(.asymmetric(insertion: .opacity.animation(.fadeIn.delay(0.25)), removal: .identity))
                    }
                }
                .offset(x: rect.minX, y: rect.minY)
        }
    }

    private var button: some View {
        Button(place.expanded ? L.shellLess : L.actionMore, action: toggle)
            .buttonStyle(.plain)
            .font(font.weight(.semibold))
            .foregroundStyle(Palette.silk.opacity(dimmed ? 0.55 : 1))
            .padding(.leading, place.expanded ? 0 : 28)
            // The text fades out over the 28 pt before the word, which sits on the row's own
            // ground alone.
            .background {
                if !place.expanded {
                    ZStack { Palette.ground; fill }
                        .mask {
                            HStack(spacing: 0) {
                                LinearGradient(colors: [.clear, .black], startPoint: .leading, endPoint: .trailing)
                                    .frame(width: 28)
                                Color.black
                            }
                        }
                }
            }
            .contentShape(.rect.inset(by: -6))
            .pointerHover()
            // The page's menu is More too: this one says what it does.
            .accessibilityLabel(place.expanded ? L.shellLess : L.titleSynopsisMore)
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
/// protected episode's still stays blurred and its text out until Show synopsis is chosen. The
/// preview is drawn apart from the page, without its environment: the row hands it the store and
/// the navigator, and says what is hidden.
struct EpisodePreview: View {
    let entry: Entry
    let row: EpisodeRow
    let hidden: Bool

    var body: some View {
        let next = entry.progress?.next ?? 1
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
    /// Go to episode, the menu's last item after a divider; nil leaves it out.
    var goTo: (() -> Void)?
    let pick: (Int) -> Void

    var body: some View {
        let current = plan.ranges[plan.index]
        return Menu {
            ForEach(plan.ranges.indices, id: \.self) { index in
                Toggle(
                    EpisodePlan.item(plan.ranges[index], in: rows),
                    isOn: Binding(get: { index == plan.index }, set: { _ in pick(index) })
                )
            }
            if let goTo {
                Divider()
                Button(L.episodeGoTo, systemImage: "arrow.right.to.line", action: goTo)
                    #if os(macOS)
                    .keyboardShortcut("l", modifiers: .command)
                    #endif
            }
        } label: {
            HStack(spacing: 6) {
                Text(EpisodePlan.title(current, in: rows)).monospacedDigit()
                Image(systemName: "chevron.up.chevron.down")
                    .font(.ui(phone ? 12 : 9, weight: .semibold))
            }
            .font(phone ? .ui(15) : .mac(12))
            .foregroundStyle(.silk)
            .padding(.horizontal, phone ? 12 : 10)
            .padding(.vertical, 4)
            .frame(minHeight: phone ? 32 : 24)
            .overlay(Capsule().strokeBorder(.faintRule))
            .contentShape(.capsule)
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .fixedSize()
        .pointerHover()
        .accessibilityLabel(L.episodeRangeLabel)
        .accessibilityValue(EpisodePlan.title(current, in: rows))
    }
}
