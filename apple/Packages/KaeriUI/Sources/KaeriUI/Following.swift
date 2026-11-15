// The people the viewer follows on AniList, and where they are in a show: a section at the end of
// the details column on the Mac and the iPad, one row under the list status on an iPhone that
// opens a sheet of all of them, and marks on the episode rows where their progress stops. Scores
// show only in the section and the sheet, never on a mark.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// How a person followed reads: the line under their name, their score, the names on a phone's
/// row and the sentence a mark says.
enum FollowWords {
    /// "Watching · Episode 8" for Watching, Paused and Dropped with some progress; the status
    /// alone otherwise. Empty for a status this build does not know.
    static func line(_ person: FollowedEntry) -> String {
        guard let status = person.status else { return "" }
        let moving = status == .watching || status == .paused || status == .dropped
        guard moving, person.progress > 0 else { return status.title }
        return L.moreFollowedProgress(status: status.title, progress: L.episodeNumber(number: Int(person.progress)))
    }

    /// The score out of ten, from AniList's 100: none when they gave none or it rounds to nought.
    static func score(_ person: FollowedEntry) -> Int? {
        guard let score = person.score else { return nil }
        let tens = Int((Double(score) / 10).rounded())
        return tens > 0 ? tens : nil
    }

    /// The person as a screen reader says them: their name, the line, then "Score 8 out of 10".
    static func spoken(_ person: FollowedEntry) -> String {
        let line = line(person)
        return Spoken.join([person.name] + (line.isEmpty ? [] : [line]) + (score(person).map { [L.followingScoreSpoken(score: $0)] } ?? []))
    }

    /// The second line of a phone's row: one name, two, or two and how many more.
    static func names(_ names: [String]) -> String {
        switch names.count {
        case 0: ""
        case 1: names[0]
        case 2: L.followingNamesTwo(first: names[0], second: names[1])
        default: L.followingNames(first: names[0], second: names[1], more: names.count - 2)
        }
    }

    /// "Ines, Lumi and Oskar watched up to here.", for a mark's tooltip and its row's value.
    /// Never a score.
    static func marked(_ marks: [FollowedMark]) -> String? {
        guard !marks.isEmpty else { return nil }
        return L.followingMarkSpoken(names: marks.map(\.name).formatted(.list(type: .and).locale(AirTime.locale)))
    }

    /// The marks of several rows in one, in order, each person once: a folded watched run's.
    static func union(_ rows: some Sequence<EpisodeRow>) -> [FollowedMark] {
        var seen = Set<String>()
        return rows.flatMap(\.followed).filter { seen.insert($0.name).inserted }
    }
}

extension AppStore {
    /// Whether title pages ask AniList for the people followed: signed in to AniList, with this
    /// device's switch on. Until the settings land the switch counts as on, its default.
    var followingAsked: Bool {
        guard let anilist = accounts.tracker("anilist"), anilist.connected, !anilist.needsReauth else { return false }
        return settings.row(.showFollowing).map(\.isOn) ?? true
    }
}

// MARK: - Pictures

/// A person's picture in a circle, through the cover art's cache; with none, the first letter of
/// their name on the selected raised fill.
struct PersonPicture: View {
    @Environment(\.displayScale) private var scale
    let name: String
    let url: String?
    let size: CGFloat

    var body: some View {
        ZStack {
            Circle().fill(.raisedSelected)
            if let url {
                ArtworkView(url: url, maxPixelSize: Int(size * scale), flat: Palette.raisedSelected)
            } else {
                Text(name.first.map { String($0).uppercased(with: AirTime.locale) } ?? "")
                    .font(.system(size: (size * 0.46).rounded(), weight: .semibold))
                    .foregroundStyle(.silk)
            }
        }
        .frame(width: size, height: size)
        .clipShape(.circle)
        .accessibilityHidden(true)
    }
}

/// Up to three pictures overlapping, each in a ring of the ground they sit on, then "+2" for the
/// rest. With Increase Contrast the ring takes the lifted edge colour, so the pictures stay apart.
struct PictureStack: View {
    @Environment(\.colorSchemeContrast) private var contrast
    let people: [(name: String, url: String?)]
    let size: CGFloat
    /// The colour of what the stack sits on.
    let ring: Color

    var body: some View {
        let shown = people.prefix(3)
        let more = people.count - shown.count
        let edge = contrast == .increased ? Palette.edge : ring
        let inset = (size * 0.18).rounded()
        HStack(spacing: -6) {
            ForEach(Array(shown.enumerated()), id: \.offset) { _, person in
                PersonPicture(name: person.name, url: person.url, size: size)
                    .background(Circle().fill(edge).padding(-2))
            }
            if more > 0 {
                Text(L.followingMarkMore(count: more))
                    .font(.system(size: max(10, (size * 0.44).rounded()), weight: .semibold))
                    .monospacedDigit()
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                    .fixedSize()
                    .padding(.leading, inset + 5)
                    .padding(.trailing, inset)
                    .frame(minWidth: size, minHeight: size, maxHeight: size)
                    .background(.raisedSelected, in: .capsule)
                    .background(Capsule().fill(edge).padding(-2))
            }
        }
        .fixedSize()
        .accessibilityHidden(true)
    }
}

// MARK: - Marks on episode rows

/// The sizes of the marks' column at an episode row's trailing end.
struct MarkMetrics {
    /// One picture's size.
    let size: CGFloat
    /// The column's width, which holds three pictures and +N.
    let column: CGFloat

    static var current: MarkMetrics {
        #if os(macOS)
        MarkMetrics(size: 20, column: 64)
        #else
        MarkMetrics(size: 24, column: 80)
        #endif
    }
}

/// The marks of one episode row, or of a folded run of rows: who stopped there, at the column's
/// trailing edge. On the Mac the pointer resting on them names everyone; a screen reader hears
/// the same sentence in the row's value, so the marks themselves are hidden from it.
struct FollowedMarks: View {
    let marks: [FollowedMark]
    var metrics = MarkMetrics.current
    /// The colour of the row the marks sit on, for the pictures' rings.
    var ring: Color = Palette.ground

    var body: some View {
        ZStack(alignment: .trailing) {
            if !marks.isEmpty {
                PictureStack(people: marks.map { ($0.name, $0.avatarUrl) }, size: metrics.size, ring: ring)
                    #if os(macOS)
                    .tooltip(FollowWords.marked(marks) ?? "")
                    #endif
                    .transition(.asymmetric(insertion: .opacity.animation(.fadeIn), removal: .opacity.animation(.fadeOut)))
            }
        }
        .frame(width: metrics.column, height: metrics.size, alignment: .trailing)
        .animation(.fadeIn, value: marks.map(\.name))
        .accessibilityHidden(true)
    }
}

// MARK: - The section, on the Mac and the iPad

/// The section's sizes: the Mac's, and the iPad's at touch sizes.
private struct SectionMetrics {
    let picture: CGFloat
    let row: CGFloat
    let gap: CGFloat
    let name: Font
    let line: Font
    let heading: Font
    let button: CGFloat
    let buttonFont: CGFloat
    let buttonGap: CGFloat

    static var current: SectionMetrics {
        #if os(macOS)
        SectionMetrics(
            picture: 28, row: 40, gap: 10, name: .mac(13, weight: .medium), line: .mac(12),
            heading: .mac(13, weight: .semibold), button: 28, buttonFont: 12, buttonGap: 8
        )
        #else
        SectionMetrics(
            picture: 36, row: 52, gap: 12, name: .ui(17), line: .ui(15),
            heading: .ui(17, weight: .semibold), button: 36, buttonFont: 15, buttonGap: 12
        )
        #endif
    }
}

/// People you follow at the end of the details column: the heading, "From AniList", five people,
/// then Show all 10, which grows the list in place. An answer inside 600 ms fades in with no
/// placeholder; a slower one shows two placeholder rows from 600 ms, held at least 0.3 s, which
/// crossfade to the people or fade out when nobody followed has the show. With nobody, the
/// switch off or AniList signed out, the section is left out.
struct FollowingSection: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let entry: Entry

    private enum Phase { case none, placeholders, people }

    /// What the section shows.
    @State private var phase = Phase.none
    /// When the placeholders came, so they stay at least 0.3 s.
    @State private var since: ContinuousClock.Instant?
    /// Whether the people came in place of placeholders: they crossfade in on standard then,
    /// and fade in quickly otherwise.
    @State private var replacing = false
    /// The title the section was last settled for: a title opened with its people here shows
    /// them at once.
    @State private var settledFor: AnilistId?
    /// Whether Show all was pressed, for this title.
    @State private var expanded = false

    /// What the section waits for: AniList's answer for the page.
    private struct Asked: Equatable {
        let id: AnilistId
        let waiting: Bool
        let people: [String]
    }

    private var people: [FollowedEntry] { entry.detail?.following ?? [] }

    /// The page is asked for, or only its kept header is here, while the people would come with it.
    private var waiting: Bool {
        store.followingAsked && (entry.detail == nil || entry.detail?.partial == true)
    }

    private var reduced: Bool { reduceMotion || navigator.motionReduced }

    var body: some View {
        let metrics = SectionMetrics.current
        ZStack(alignment: .topLeading) {
            if phase == .placeholders {
                placeholders(metrics)
                    .transition(.opacity.animation(.fadeOut))
            }
            if phase == .people, !people.isEmpty {
                list(metrics)
                    .transition(.asymmetric(
                        insertion: .opacity.animation(replacing ? .standard : .fadeIn),
                        removal: .opacity.animation(.fadeOut)
                    ))
            }
        }
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .task(id: Asked(id: entry.id, waiting: waiting, people: people.map(\.name))) { await settle() }
    }

    // MARK: Parts

    private func list(_ metrics: SectionMetrics) -> some View {
        let first = people.prefix(5)
        let rest = people.dropFirst(5)
        return VStack(alignment: .leading, spacing: 0) {
            Text(L.moreFollowing)
                .font(metrics.heading)
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            Text(L.moreFollowingNote)
                .font(metrics.line)
                .foregroundStyle(.haze)
                .padding(.bottom, 6)
            ForEach(Array(first.enumerated()), id: \.offset) { _, person in
                FollowedPersonRow(person: person, metrics: metrics)
            }
            if !rest.isEmpty {
                // The rest grow in place under the five, on standard, and fade in; the button
                // goes as they come. Nothing above them moves.
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(rest.enumerated()), id: \.offset) { _, person in
                        FollowedPersonRow(person: person, metrics: metrics)
                    }
                }
                .animation(.fadeIn) { $0.opacity(expanded ? 1 : 0) }
                .frame(height: expanded ? nil : 0, alignment: .top)
                .clipped()
                .accessibilityHidden(!expanded)
                Button(L.followingShowAll(count: people.count)) {
                    if reduced { expanded = true } else { withAnimation(.standard) { expanded = true } }
                }
                .buttonStyle(SecondaryCapsuleStyle(height: metrics.button, fontSize: metrics.buttonFont))
                .pointerHover()
                .padding(.top, metrics.buttonGap)
                .animation(.fadeOut) { $0.opacity(expanded ? 0 : 1) }
                .frame(height: expanded ? 0 : nil, alignment: .top)
                .clipped()
                .disabled(expanded)
                .accessibilityHidden(expanded)
            }
        }
        // Twenty-four from More about this show, with the column's own spacing.
        .padding(.top, 6)
    }

    private func placeholders(_ metrics: SectionMetrics) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            bar(width: 120, height: 11).padding(.top, 4).padding(.bottom, 9)
            bar(width: 72, height: 8).padding(.bottom, 8)
            ForEach(0..<2, id: \.self) { _ in
                HStack(spacing: metrics.gap) {
                    Circle().fill(.raised).frame(width: metrics.picture, height: metrics.picture)
                    VStack(alignment: .leading, spacing: 6) {
                        bar(width: 96, height: 9)
                        bar(width: 140, height: 8)
                    }
                }
                .frame(height: metrics.row)
            }
        }
        .padding(.top, 6)
        .accessibilityHidden(true)
    }

    private func bar(width: CGFloat, height: CGFloat) -> some View {
        RoundedRectangle(cornerRadius: 4).fill(.raised).frame(width: width, height: height)
    }

    // MARK: Timing

    private func settle() async {
        let fresh = settledFor != entry.id
        settledFor = entry.id
        if fresh { expanded = false }
        let target: Phase = people.isEmpty ? .none : .people
        if waiting {
            // Another title's people go at once; this one's wait 600 ms for a placeholder.
            if phase == .people { quietly { phase = .none } }
            guard phase != .placeholders else { return }
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            replacing = false
            change(.fadeIn) { phase = .placeholders }
            since = .now
            return
        }
        if phase == .placeholders {
            // Held at least 0.3 s, then one crossfade to the people in place, or a fade out.
            if let since {
                let held = ContinuousClock.now - since
                if held < .milliseconds(300) { try? await Task.sleep(for: .milliseconds(300) - held) }
                guard !Task.isCancelled else { return }
            }
            replacing = target == .people
            change(target == .people ? .standard : .fadeOut) { phase = target }
        } else if fresh {
            // A page opened with its answer here shows it with the page.
            replacing = false
            quietly { phase = target }
        } else if phase != target {
            replacing = false
            change(target == .people ? .fadeIn : .fadeOut) { phase = target }
        }
    }

    /// A change whose room animates, unless Reduce Motion asks for fades only: the room then
    /// changes at once and the parts fade by their own transitions.
    private func change(_ animation: Animation, _ body: () -> Void) {
        if reduced { body() } else { withAnimation(animation, body) }
    }

    private func quietly(_ body: () -> Void) {
        var quiet = Transaction()
        quiet.disablesAnimations = true
        withTransaction(quiet, body)
    }
}

/// One person in the section: their picture, their name with the line under it, and their score
/// at the row's end. One element for a screen reader, which hears the score in words. Nothing
/// happens on a person.
private struct FollowedPersonRow: View {
    let person: FollowedEntry
    let metrics: SectionMetrics

    var body: some View {
        let line = FollowWords.line(person)
        HStack(spacing: metrics.gap) {
            PersonPicture(name: person.name, url: person.avatarUrl, size: metrics.picture)
            VStack(alignment: .leading, spacing: 0) {
                Text(person.name)
                    .font(metrics.name)
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                if !line.isEmpty {
                    Text(line)
                        .font(metrics.line)
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let score = FollowWords.score(person) {
                Text(L.moreFollowedScore(score: score))
                    .font(metrics.line)
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .fixedSize()
            }
        }
        .frame(minHeight: metrics.row)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(FollowWords.spoken(person))
    }
}

// MARK: - The row and the sheet, on an iPhone

#if os(iOS)
/// People you follow under the list status row: the pictures, the heading and the first names,
/// one button that opens the sheet of all of them. Left out when nobody followed has the show.
struct FollowingPhoneRow: View {
    let entry: Entry
    @State private var open = false

    var body: some View {
        let people = entry.detail?.following ?? []
        let names = FollowWords.names(people.map(\.name))
        Button { open = true } label: {
            HStack(spacing: 12) {
                PictureStack(people: people.map { ($0.name, $0.avatarUrl) }, size: 28, ring: Palette.raised)
                VStack(alignment: .leading, spacing: 0) {
                    Text(L.moreFollowing)
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                    Text(names)
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Image(systemName: "chevron.right")
                    .font(.ui(14, weight: .semibold))
                    .foregroundStyle(.haze)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .frame(minHeight: 64)
            .background(.raised, in: .rect(cornerRadius: 22))
            .overlay(RoundedRectangle(cornerRadius: 22).strokeBorder(Palette.glassEdge))
            .contentShape(.rect(cornerRadius: 22))
        }
        .buttonStyle(FollowingRowStyle())
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.moreFollowing)
        .accessibilityValue(names)
        .accessibilityHint(L.followingOpenHelp)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { open = true }
        .sheet(isPresented: $open) {
            FollowingSheet(entry: entry)
                .presentationDetents(people.count > 5 ? [.large] : [.medium, .large])
                .presentationDragIndicator(.visible)
        }
    }
}

/// The row darkens a touch while pressed, as list rows do.
private struct FollowingRowStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .overlay(RoundedRectangle(cornerRadius: 22).fill(configuration.isPressed ? Palette.rowPressed : .clear))
    }
}

/// Everyone followed who has the show: picture, name, the line under it and the score, with
/// Done. Done or a swipe closes it, and focus goes back to the row.
struct FollowingSheet: View {
    @Environment(\.dismiss) private var dismiss
    let entry: Entry

    var body: some View {
        let people = entry.detail?.following ?? []
        NavigationStack {
            List {
                Section {
                    ForEach(Array(people.enumerated()), id: \.offset) { _, person in
                        SheetPersonRow(person: person)
                            .listRowBackground(Palette.raised)
                            .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
                    }
                } header: {
                    Text(L.moreFollowingNote)
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        .textCase(nil)
                        .frame(maxWidth: .infinity)
                        .padding(.bottom, 4)
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .environment(\.defaultMinListRowHeight, 60)
            .background(.ground)
            .navigationTitle(L.moreFollowing)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.actionDone) { dismiss() }
                        .fontWeight(.semibold)
                        .tint(.silk)
                }
            }
        }
        .presentationBackground(Palette.ground)
    }
}

/// One person in the sheet.
private struct SheetPersonRow: View {
    let person: FollowedEntry

    var body: some View {
        let line = FollowWords.line(person)
        HStack(spacing: 12) {
            PersonPicture(name: person.name, url: person.avatarUrl, size: 40)
            VStack(alignment: .leading, spacing: 2) {
                Text(person.name).font(.ui(17)).foregroundStyle(.silk).lineLimit(1)
                if !line.isEmpty {
                    Text(line).font(.ui(15)).monospacedDigit().foregroundStyle(.haze).lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let score = FollowWords.score(person) {
                Text(L.moreFollowedScore(score: score))
                    .font(.ui(17))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .fixedSize()
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(FollowWords.spoken(person))
    }
}
#endif
