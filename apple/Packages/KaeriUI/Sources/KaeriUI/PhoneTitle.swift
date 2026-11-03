// A title's page on iPhone and iPad, as PhoneTitle draws it.

#if os(iOS)
import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

struct PhoneTitle: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var typeSize
    @Namespace private var rotor
    /// Whether the main button has scrolled away, for the capsule that takes its place below.
    @State private var mainButtonGone = false
    /// Whether the episode rows keep room for text, settled when they are first laid out.
    @State private var room = EpisodeRoom()
    let id: AnilistId

    /// Case B: the service the main action opens.
    private var serviceMain: ServiceLink? {
        guard let availability = store.licensed.availability[id], availability.case == .onService else { return nil }
        return availability.main
    }

    var body: some View {
        let entry = store.titles.page(id)
        let action = TitleAction(entry, handedOff: navigator.handedOff, episodes: store.titles.episodes[entry.id])
        let art = store.heroArt(entry)
        let episodes = entry.episodeTable(store.titles.episodes[id] ?? [])
        let plan = EpisodePlan(rows: episodes, picked: navigator.episodeRange[id])
        let loaded = store.titles.episodes[id] != nil
        // A list, so the episodes take their swipes (native-ux), drawn as the scroll view was.
        ScrollViewReader { reader in
        List {
            VStack(alignment: .leading, spacing: 0) {
                // The wide art: it stretches when pulled down and scrolls at half speed (T1).
                // With none, the page starts under the navigation bar, the cover beside the title.
                if !art.isCompact {
                    HeroArtView(art: art, entry: entry)
                        .frame(height: 250)
                        // The ground fading up from the art's foot, under the cover and a title
                        // of three lines.
                        .overlay(alignment: .bottom) { BannerScrim(held: 38, height: 82) }
                        .visualEffect { [reduceMotion] content, proxy in
                            let y = proxy.frame(in: .scrollView(axis: .vertical)).minY
                            let h = proxy.size.height
                            return content
                                .scaleEffect(reduceMotion || y <= 0 ? 1 : (h + y) / h, anchor: .bottom)
                                .offset(y: reduceMotion || y >= 0 ? 0 : -y * 0.5)
                        }
                        .clipped()
                }
                HStack(alignment: .bottom, spacing: 16) {
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 400)
                        .frame(width: 100, height: 150)
                        .clipShape(.rect(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassEdge.opacity(1.25)))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(entry.showTitle)
                            .font(.showTitle(size: 22, relativeTo: .title2))
                            .foregroundStyle(.silk)
                            // As large as `.accessibility3`, in four lines at most.
                            .lineLimit(4)
                            .dynamicTypeSize(...DynamicTypeSize.accessibility3)
                            .titleOverArt()
                            .accessibilityAddTraits(.isHeader)
                        OtherTitlesLine(entry: entry, phone: true)
                        Text(entry.phoneMetadata)
                            .groupHeaderStyle()
                            .padding(.top, 2)
                    }
                    .padding(.bottom, 4)
                }
                .padding(.horizontal, 16)
                // Over the art's foot, except at the accessibility sizes, where the title would
                // grow up into the art and lose its contrast.
                .padding(.top, art.isCompact || typeSize.isAccessibilitySize ? 8 : -60)

                VStack(alignment: .leading, spacing: 16) {
                    // No source has it: nothing to play, and the way to one in Play's place.
                    if let main = serviceMain {
                        Button { store.openService(entry.id, link: main, episode: nil) } label: {
                            HStack(spacing: 8) {
                                Text(L.licensedWatchOn(service: main.service))
                                Image(systemName: "arrow.up.right").font(.ui(13, weight: .semibold))
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                        .accessibilityHint(L.licensedOpensOutside)
                    } else if action.playable, store.match.asks(entry.id) {
                        ChooseMatchButton(id: entry.id, height: 50, fontSize: 17, fills: true)
                        MatchUncertainLine(id: entry.id, font: .ui(14))
                    } else if action.playable, store.titles.unlisted(entry.id) {
                        AddSourceButton(sentence: true, height: 50, fontSize: 17, fills: true)
                    } else if action.playable {
                        Button {
                            action.play(entry.id, store: store, navigator: navigator)
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.ui(13))
                                // One line that shrinks a little for a longer translation; wrapped at the
                                // accessibility sizes. Never wider than the screen.
                                Text(action.sentence)
                                    .lineLimit(typeSize.isAccessibilitySize ? nil : 1)
                                    .minimumScaleFactor(typeSize.isAccessibilitySize ? 1 : 0.7)
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                        .contextMenu { FromStartItem(id: entry.id, action: action) }
                        .onScrollVisibilityChange(threshold: 0.2) { visible in mainButtonGone = !visible }
                    } else {
                        // Still to air: when, in the button's place.
                        AirsStatus(text: action.sentence, height: 50, fontSize: 17, fills: true)
                    }

                    HStack(spacing: 12) {
                        Menu {
                            StatusItems(id: entry.id)
                        } label: {
                            HStack(spacing: 6) {
                                Text(entry.listStatus.map { $0.label.capitalizedSentence } ?? L.libraryAddToList)
                                Image(systemName: "chevron.up.chevron.down").font(.ui(11, weight: .semibold))
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
                        if entry.trailerUrl != nil {
                            Button {
                                navigator.playTrailer(entry, store: store)
                            } label: {
                                Text(L.titleTrailer).frame(maxWidth: .infinity)
                            }
                            .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
                        }
                        if action.playable, !store.titles.unlisted(entry.id), store.tv.available {
                            Button {
                                store.playOnTV(entry.id, episode: String(action.episode), from: action.resume?.position)
                            } label: {
                                Image(systemName: "tv").frame(width: 20)
                            }
                            .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
                            .accessibilityLabel(L.tvPlayOn)
                        }
                    }
                    LicensedLines(id: entry.id, font: .ui(14))

                    VStack(alignment: .leading, spacing: 12) {
                        if !entry.shownSynopsis.isEmpty {
                            // Two lines, and "More" shows the rest in place.
                            InlineMoreText(text: entry.shownSynopsis, lines: 2, font: .ui(15))
                                .id(entry.id)
                                // Read after the episodes: title, metadata, the main button,
                                // progress, episodes, then the synopsis.
                                .accessibilitySortPriority(-1)
                        }
                        if entry.isDemo { DemoCredit() }
                        // The facts and the top tags, a line each, cut at the end.
                        VStack(alignment: .leading, spacing: 4) {
                            if let facts = entry.factsLine() {
                                Text(facts).lineLimit(1)
                            }
                            if !entry.topTags.isEmpty {
                                Text(entry.topTags.joined(separator: " · ")).lineLimit(1)
                            }
                        }
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        MoreAboutRow(entry: entry)
                    }

                    let family = TitleFamily(entry)
                    if !family.seasons.isEmpty {
                        // The season on screen in the middle as the page opens, so a long
                        // franchise's later seasons open with their own chip in view.
                        ScrollViewReader { proxy in
                            ScrollView(.horizontal) {
                                HStack(spacing: 8) {
                                    ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                                        seasonChip(index: index, season: season, entry: entry).id(season.id)
                                    }
                                }
                                .padding(.horizontal, 16)
                            }
                            .scrollIndicators(.never)
                            .padding(.horizontal, -16)
                            .onAppear { proxy.scrollTo(entry.id, anchor: .center) }
                        }
                    }
                    if entry.detail != nil {
                        let cards = entry.relatedCards(excluding: family.picked)
                        if !cards.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(L.titleRelated).font(.ui(15)).foregroundStyle(.haze)
                                    .accessibilityAddTraits(.isHeader)
                                ScrollView(.horizontal) {
                                    HStack(spacing: 4) {
                                        ForEach(cards, id: \.id) { linked in
                                            RelatedCard(linked: linked, cover: CGSize(width: 40, height: 60), phone: true)
                                        }
                                    }
                                    .padding(.horizontal, 12)
                                }
                                .scrollIndicators(.never)
                                .padding(.horizontal, -16)
                            }
                        }
                    } else if !family.related.isEmpty {
                        HStack(spacing: 8) {
                            Text(L.titleRelated).font(.ui(13)).foregroundStyle(.haze)
                            ForEach(family.related, id: \.id) { related in
                                Button {
                                    navigator.openTitle(related.id)
                                } label: {
                                    HStack(spacing: 6) {
                                        Text(related.format.map(TitleWords.format) ?? related.title)
                                            .fontWeight(.medium)
                                            .foregroundStyle(.silk)
                                        Text(related.relation.capitalizedSentence).foregroundStyle(.haze)
                                    }
                                    .font(.ui(15))
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 4)
                                    .frame(minHeight: 32)
                                    .overlay(Capsule().strokeBorder(.faintRule))
                                    .contentShape(.capsule)
                                }
                                .buttonStyle(.plain)
                                .pointerHover()
                            }
                        }
                    }
                }
                .accessibilityElement(children: .contain)
                .padding(.horizontal, 16)
                .padding(.top, 16)
                .padding(.bottom, 16)
            }
            .bareRow()
            Color.clear.frame(height: 0).bareRow().id("episodes")
            PhoneEpisodes(
                entry: entry, spoilers: store.spoilerProtection(for: id), rotor: rotor,
                room: room.room, rows: episodes, plan: plan
            )
            Color.clear.frame(height: 32).bareRow()
        }
        .bareList()
        // The placeholders give way to the rows in one fade (T8), and a range switch crossfades
        // its rows in place.
        .animation(.standard, value: loaded)
        .animation(.standard, value: plan.index)
        // The room for text is settled when the rows are first laid out, and again each time the
        // page comes back or a range is picked: text that turned up meanwhile waits for that.
        .onChange(of: loaded, initial: true) { if loaded { room.settle(plan.shown) } }
        .onChange(of: plan.index) { room.settle(plan.shown, again: true) }
        .onAppear { if loaded { room.settle(plan.shown, again: true) } }
        .onDisappear { navigator.endEpisodeVisit(id) }
        .nudgeableScroll()
        .ignoresSafeArea(edges: art.isCompact ? [] : .top)
        .scrollIndicators(.never)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[id] ?? [], in: rotor)
        .handsOff(title: entry)
        // The main button scrolled away: a capsule of it fades in at the foot (native-ux, iPhone).
        .safeAreaInset(edge: .bottom) {
            if mainButtonGone, action.playable, !store.titles.unlisted(entry.id) {
                Button {
                    action.play(entry.id, store: store, navigator: navigator)
                } label: {
                    Label(action.short, systemImage: "play.fill")
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                .contextMenu { FromStartItem(id: entry.id, action: action) }
                .padding(.bottom, 8)
                .transition(.opacity)
            }
        }
        .animation(reduceMotion ? nil : .standard, value: mainButtonGone)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    TitleMoreItems(entry: entry)
                } label: {
                    Label(L.actionMore, systemImage: "ellipsis")
                }
            }
        }
        .navigationBarTitleDisplayMode(.inline)
        .trailerLaunchSheet(entry)
        .task(id: id) {
            navigator.focusedTitle = id
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            store.loadDub(id)
            // What the show sets for itself, its spoiler protection among it: from the database.
            store.loadShowSettings(id)
        }
        // A screenshot's `-KaeriScroll`: the page scrolled to its episodes once they are drawn.
        .task(id: loaded) {
            guard loaded, navigator.launchScroll != nil else { return }
            navigator.launchScroll = nil
            try? await Task.sleep(for: .milliseconds(1_200))
            reader.scrollTo("episodes", anchor: UnitPoint(x: 0, y: 0.12))
        }
        }
    }

    private func seasonChip(index: Int, season: TitleFamily.Season, entry: Entry) -> some View {
        Button {
            if !season.isThis { navigator.openTitle(season.id) }
        } label: {
            HStack(spacing: 6) {
                Text(L.seasonNumber(number: index + 1)).fontWeight(season.isThis ? .semibold : .medium)
                    .foregroundStyle(season.isThis ? .amber : .silk)
                if season.isThis, let done = entry.progress?.done, let of = entry.progressOf {
                    Text("\(done) / \(of.total)").foregroundStyle(.haze)
                        .contentTransition(.numericText(value: Double(done)))
                        .animation(.standard, value: done)
                }
            }
            .font(.ui(15))
            .padding(.horizontal, 14)
            // Taller than 32 points only when larger text needs it.
            .padding(.vertical, 4)
            .frame(minHeight: 32)
            .background(season.isThis ? Palette.selection : .clear, in: .capsule)
            .overlay(Capsule().strokeBorder(season.isThis ? .clear : Palette.faintRule))
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .pointerHover()
        // Pointed at, on an iPad in a narrow window, its page and art asked for ahead.
        .onHover { inside in if inside, !season.isThis { store.prefetchPage(season.id) } }
    }
}

/// The episodes on an iPhone's page, as rows of its list: a row each, the next one marked, stills
/// after it blurred.
struct PhoneEpisodes: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let spoilers: Bool
    /// Where the page's Unwatched episodes rotor finds the rows.
    let rotor: Namespace.ID
    /// Whether the table keeps room for text: settled by the page when it lays the rows out.
    let room: Bool
    let rows: [EpisodeRow]
    let plan: EpisodePlan

    var body: some View {
        let loaded = store.titles.episodes[entry.id] != nil
        // Rows of the page's list, each with its swipes; the folded run is one row, which opens
        // in place.
        let inset = EdgeInsets(top: 0, leading: 16, bottom: 0, trailing: 16)
        let dub = store.titles.dubLines[entry.id]
        let ranged = !plan.ranges.isEmpty
        let protectedFrom = EpisodeProtection.startsAt(
            plan.shown, next: entry.progress?.next ?? 1, spoilers: spoilers
        )
        Group {
            if loaded {
                // A long series: the dub line and the range menu share a row.
                if ranged {
                    HStack(spacing: 12) {
                        if let dub {
                            Text(dub)
                                .font(.ui(15))
                                .foregroundStyle(.haze)
                                .lineLimit(1)
                                .transition(.opacity.animation(.fadeIn))
                        }
                        Spacer(minLength: 0)
                        EpisodeRangeMenu(plan: plan, rows: rows, phone: true) {
                            navigator.episodeRange[entry.id] = $0
                        }
                    }
                    .frame(minHeight: 44)
                    .bareRow(inset)
                }
                // The watched run folds away above the last episode watched.
                if let fold = plan.fold {
                    WatchedFoldSection(
                        fold: fold, shown: navigator.watchedShown(entry.id), phone: true,
                        dub: ranged ? nil : dub,
                        toggle: { navigator.toggleWatched(entry.id) }
                    ) {
                        ForEach(plan.shown.prefix(fold.count), id: \.number) {
                            PhoneEpisodeRow(entry: entry, row: $0, spoilers: spoilers, rotor: rotor, room: room, protectedFrom: protectedFrom)
                        }
                    }
                    .bareRow(inset)
                }
                // With no watched run to fold, the line has a row of its own above the episodes.
                if plan.fold == nil, !ranged, let dub {
                    Text(dub)
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                        .bareRow(inset)
                        .transition(.opacity.animation(.fadeIn))
                }
                ForEach(plan.shown.dropFirst(plan.fold?.count ?? 0), id: \.number) {
                    PhoneEpisodeRow(entry: entry, row: $0, spoilers: spoilers, rotor: rotor, room: room, protectedFrom: protectedFrom)
                        .bareRow(inset)
                        .transition(.opacity)
                }
            } else {
                EpisodePlaceholders(count: entry.episodes, metrics: .phone)
                    .bareRow(inset)
            }
        }
    }
}

/// One episode on an iPhone's page.
struct PhoneEpisodeRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let row: EpisodeRow
    let spoilers: Bool
    let rotor: Namespace.ID
    /// Whether the table keeps room for text.
    let room: Bool
    /// The episode that spoiler protection's line sits above.
    let protectedFrom: String?

    private var next: UInt32 { entry.progress?.next ?? 1 }

    private var hidden: Bool {
        EpisodeProtection.hidden(
            row, next: next, spoilers: spoilers,
            revealed: navigator.synopsisShown[entry.id] ?? [], all: navigator.synopsisAllShown.contains(entry.id)
        )
    }

    var body: some View {
        VStack(spacing: 0) {
            if row.number == protectedFrom {
                let shown = navigator.synopsisAllShown.contains(entry.id)
                ProtectionLine(episode: Int(next), shown: shown, phone: true) {
                    withAnimation(.standard) { navigator.showAllSynopses(entry.id, !shown) }
                }
            }
            rowBody
            DiscussionLine(entry: entry, row: row, inset: 156)
        }
        .episodeSwipes(entry, row)
    }

    @ViewBuilder
    private var rowBody: some View {
        let number = UInt32(row.number) ?? 0
        let current = number == next
        if row.isUnaired(), let airs = row.airsDate {
            PhoneUnairedRow(row: row, airs: airs)
                .accessibilityRotorEntry(id: row.number, in: rotor)
        } else if row.metadataOnly || row.unsourced {
            // One no source has: nothing to play, so a tap offers what can be done, adding a
            // source among it, rather than playing.
            Menu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } label: {
                label(current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: spoken(current: current), hint: hint) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .accessibilityRotorEntry(id: row.number, in: rotor)
        } else {
            Button {
                store.send(.play(id: entry.id, episode: row.number))
            } label: {
                label(current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: spoken(current: current), hint: hint) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .accessibilityRotorEntry(id: row.number, in: rotor)
            .contextMenu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } preview: {
                EpisodePreview(entry: entry, row: row)
            }
        }
    }

    private func spoken(current: Bool) -> [String] {
        row.spokenValue(entry, current: current, upcoming: false)
    }

    /// The synopsis said last, or why it is not there.
    private var hint: String? {
        if hidden { return L.episodeSynopsisHidden }
        return room ? row.description : nil
    }

    private func label(current: Bool) -> some View {
        EpisodeRowLabel(
            entry: entry, row: row, metrics: .phone, room: room, hidden: hidden, current: current,
            numberColumn: false, asked: navigator.synopsisIsShown(entry.id, row.number)
        )
        .opacity(row.unsourced ? 0.55 : 1)
        .contentShape(.rect)
    }
}

/// The next episode still to air on an iPhone's page: no button; the flat placeholder, its title
/// in haze and "Airs Thursday 16:30" in water.
struct PhoneUnairedRow: View {
    let row: EpisodeRow
    let airs: Date

    var body: some View {
        let when = AirsWords.airs(airs)
        HStack(alignment: .top, spacing: 12) {
            RoundedRectangle(cornerRadius: 6).fill(Palette.artPlaceholder)
                .frame(width: EpisodeMetrics.phone.still.width, height: EpisodeMetrics.phone.still.height)
            VStack(alignment: .leading, spacing: 4) {
                Text(row.title ?? L.episodeNumberString(number: row.number))
                    .font(.ui(16, weight: .semibold)).foregroundStyle(.haze).lineLimit(2)
                HStack(spacing: 6) {
                    Circle().fill(.water).frame(width: 6, height: 6)
                    Text(when).monospacedDigit()
                }
                .font(.ui(13))
                .foregroundStyle(.water)
            }
            Spacer(minLength: 0)
        }
        .padding(.top, 8)
        .frame(minHeight: EpisodeMetrics.phone.short, alignment: .top)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.titleEpisodeRowUnaired(title: row.title.map { L.episodeTitledText(number: row.number, title: $0) } ?? L.episodeNumberString(number: row.number), when: when))
    }
}

/// "Play from the beginning", in the context menu of a Resume button: the episode from its first
/// second, its place let go. Nothing for a Start button, which starts there anyway.
struct FromStartItem: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    let action: TitleAction

    var body: some View {
        if action.resume != nil, action.playable {
            Button(L.episodePlayFromBeginning) {
                store.send(.play(id: id, episode: String(action.episode), fromStart: true))
            }
        }
    }
}

extension TitleAction {
    /// The capsule's words, with no clock: "Resume episode 7", "Start episode 4".
    var short: String {
        resume == nil ? L.titleButtonPlay(episode: Int(episode)) : L.shellResumeEpisode(episode: Int(episode))
    }
}
#endif
