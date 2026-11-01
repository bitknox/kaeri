// A title's page: what it is, its seasons, its episodes, and the one amber action.

import KaeriDesign
import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

/// The episode a title's action plays, and how it says so.
struct TitleAction: Equatable {
    let episode: UInt32
    let resume: ResumePoint?
    /// Whether the episode is out, by the title's broadcasts.
    let airing: EpisodeAvailability
    /// The episode another device handed over, which the action resumes where it was there.
    let handedOff: HandedOff?

    /// Every episode from the next on is one no source lists: nothing to play, and the button
    /// says so in the place of Play.
    let unsourced: Bool

    init(_ entry: Entry, handedOff: HandedOff? = nil, episodes: [EpisodeRow]? = nil) {
        if let handedOff, handedOff.id == entry.id, let number = UInt32(handedOff.episode) {
            episode = number
            resume = ResumePoint(position: handedOff.position, fraction: nil)
            airing = .aired
            self.handedOff = handedOff
            unsourced = false
            return
        }
        // The engine's next episode, or the first after it that some source lists.
        let target = entry.playableNext(in: episodes)
        episode = target.episode
        unsourced = target.unsourced
        resume = entry.resume
        airing = resume == nil ? entry.airing(of: episode) : .aired
        self.handedOff = nil
    }

    /// Play it: from the handed-over place, which is then used up, or as the engine would.
    @MainActor
    func play(_ id: AnilistId, store: AppStore, navigator: Navigator) {
        if let handedOff {
            store.play(id, episode: handedOff.episode, at: handedOff.position)
            navigator.handedOff = nil
        } else {
            store.send(.play(id: id, episode: String(episode)))
        }
    }

    /// Whether there is an episode to play: never one still to air, which nothing could find.
    var playable: Bool { airing.hasAired && !unsourced }

    /// "Resume episode 7 · 10:06", "Play episode 4". With the episode still to air, the status in
    /// the button's place: "Episode 10 airs Thursday 16:30".
    var title: String {
        if let resume { return L.titleButtonResume(episode: Int(episode), time: resume.clock) }
        if unsourced { return L.episodeUnsourced }
        if !playable { return airsLine }
        return L.titleButtonPlay(episode: Int(episode))
    }

    /// The same words on iPhone and the Mac: the casing is the same on both.
    var sentence: String { title }

    /// "Episode 10 airs Thursday 16:30", or "Episode 10 is not out yet" when nothing dates it.
    private var airsLine: String {
        airing.airs().map { L.episodeAirs(episode: Int(episode), when: $0) } ?? L.episodeNotOut(episode: Int(episode))
    }
}

/// A title's seasons, and what else it has: films, OVAs and specials.
///
/// The seasons are the franchise's, as the engine walks them along the prequel and sequel chain:
/// the same list, numbered the same, on every one of their pages. Until the engine has walked
/// them there are none, rather than seasons numbered from what this page alone knows, which
/// would renumber as the rest arrived.
struct TitleFamily {
    struct Season: Identifiable, Equatable {
        let id: AnilistId
        let title: String
        let isThis: Bool
    }

    let seasons: [Season]
    /// What the picker holds, or will once the engine has walked the seasons: this title's own
    /// series prequels and sequels meanwhile, kept out of Related so that none moves from there
    /// into the picker.
    let picked: Set<AnilistId>
    let related: [RelatedTitle]

    /// `this` is the season picked, when the page still shows the last one while it comes.
    init(_ entry: Entry, this: AnilistId? = nil) {
        let series: Set<String> = ["TV", "TV SHORT", "ONA"]
        let franchise = entry.detail?.seasons
        let this = this ?? entry.id
        seasons = (franchise ?? []).map { Season(id: $0.id, title: $0.title, isThis: $0.id == this) }
        let waiting: [RelatedTitle] = franchise != nil ? [] : entry.related.filter {
            ($0.relation == "prequel" || $0.relation == "sequel") && series.contains($0.format ?? "")
        }
        picked = Set(seasons.map(\.id) + waiting.map(\.id))
        related = entry.related.filter { !series.contains($0.format ?? "") }
    }
}

/// Where a title's next episode airs: "Thursday 16:30".
func nextAiring(_ entry: Entry, now: Date = .now) -> String? {
    guard let seconds = entry.nextAiringIn else { return nil }
    return CalendarWords.weekdayTime(now.addingTimeInterval(TimeInterval(seconds)), now: now)
}

/// The Mac's title page, over the section it was opened from.
struct TitlePage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Namespace private var rotor
    let id: AnilistId
    /// The title the page is laid out for: the last one opened, until the next one's is here.
    @State private var laidOut: AnilistId?
    /// The title that has been waited for long enough, laid out with what is known of it.
    @State private var waited: AnilistId?
    /// The title the page's words, episodes and facts are drawn for: the one laid out, once the
    /// last one's have faded out.
    @State private var drawn: AnilistId?
    /// The title laid out last, which a fade under way swaps to at its end.
    @State private var drawing: AnilistId?
    /// Whether those parts are faded out, between one title's and the next's.
    @State private var between = false
    /// The wide art on screen and the title it is of. Across a season switch the last season's
    /// stays until the next one's is known and here, then crossfades to it, as a visited season's
    /// does at once.
    @State private var shownArt: ShownArt?
    /// The title whose art has been waited for long enough: its art shows then, image or not.
    @State private var artWaited: AnilistId?
    /// Raised for each art shown, so the newest lies over the art it replaces.
    @State private var artLayer = 0.0
    /// The last title shown whose seasons and related titles are known: its row stays while a
    /// title picked from it shows as the card it was picked from, which knows none of them.
    @State private var familyOf: AnilistId?

    /// Which art, of which title: a change of either is a new picture, crossfaded in.
    struct ArtKey: Hashable {
        let id: AnilistId
        let art: HeroArt

        func hash(into hasher: inout Hasher) { hasher.combine(id) }
    }

    /// The pictures a title picked will show, asked for while it waits: they change as its page
    /// says more, a banner where its card had none.
    struct Coming: Hashable {
        let cover: String?
        let wide: String?
    }

    /// The art on screen, with the title it was resolved for.
    struct ShownArt {
        let key: ArtKey
        let entry: Entry
    }

    /// The next art fades in over the last, which stays whole beneath until it is covered.
    static let artSwap = AnyTransition.asymmetric(
        insertion: .opacity,
        removal: AnyTransition.opacity.animation(.fadeIn.delay(0.2))
    )

    /// How long a season or related title picked waits for its page before it shows what is
    /// known of it. Meanwhile the last one's page stays, and only the selection moves.
    static let waitFor: Duration = .milliseconds(150)

    var body: some View {
        // A season picked on the page moves the selection at once. When its page is here within
        // a moment it shows whole, in one change. Past that it shows what its picker's card
        // knows, its title and cover, and keeps room for the rest, each part coming in once as
        // it arrives: never the card's version first and then again as itself. Its page is its
        // title, its episodes, what the services say, its cover and its wide art.
        let ready = laidOut == nil || laidOut == id || waited == id || Self.pageHere(id, in: store)
        let laid = store.titles.page(ready ? id : laidOut ?? id)
        // The words, episodes and facts change title by fading through the ground (`between`):
        // the last title's leave, then the next one's come in where they stood, so two titles'
        // words are never on screen at once and the page never dissolves whole.
        let entry = drawn.map { $0 == laid.id ? laid : store.titles.page($0) } ?? laid
        // A season's stand-in, from its picker's card, knows no banner, and a cover or a banner
        // on its way would show as a plate first: the last art stays until the next is whole, or
        // until it is known there is none.
        let next = ArtKey(id: entry.id, art: store.pageArt(entry))
        let shown = shownArt.flatMap { held in
            held.key != next && !Self.artReady(next, in: store) && (artWaited != next.id || next.art == .waiting) ? held : nil
        } ?? ShownArt(key: next, entry: entry)
        let art = shown.key.art
        // A related title shown as its card knows no seasons and no related titles of its own:
        // the row it was picked from stays until its page says what is in it.
        let kin = Self.kin(of: entry, last: familyOf, in: store)
        // What the title picked shows with, asked for while the last one's page stays.
        let picked = ready ? nil : store.titles.page(id)
        let coming = Coming(cover: picked?.coverUrl, wide: picked.flatMap { store.pageArt($0).url })
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // With no wide art the page starts higher: under the toolbar, not under a banner.
                Color.clear.frame(height: art.isCompact ? 64 : 96)
                // Art flying onto the banner covers the cover and the words over it: they come in
                // once it has landed, rather than at once with the art drawn under them.
                let underFlyer = navigator.awaits(entry.id, .banner)
                TitleHero(entry: entry, faded: between)
                    .opacity(underFlyer ? 0 : 1)
                    .animation(.fadeIn, value: underFlyer)
                let family = TitleFamily(kin, this: id)
                if !family.seasons.isEmpty || !family.related.isEmpty {
                    TitleFamilyRow(entry: kin, family: family, faded: between)
                        .padding(.top, 20)
                }
                HStack(alignment: .top, spacing: 32) {
                    EpisodeList(entry: entry, rotor: rotor)
                    // The synopsis and the facts once the title itself is here: a title picked
                    // keeps their room empty meanwhile, as its card knows none of them.
                    let known = store.titles[entry.id] != nil
                    ZStack(alignment: .top) {
                        if known { TitleDetails(entry: entry).transition(.opacity) }
                    }
                    .animation(.fadeIn, value: known)
                    .frame(width: 260, alignment: .top)
                    // Another title's facts are other views: they never fade out over these.
                    .id(entry.id)
                    .transition(.identity)
                }
                .opacity(between ? 0 : 1)
                .padding(.top, 24)
            }
            .padding(EdgeInsets(top: 0, leading: 24, bottom: 20, trailing: 28))
            // On the Mac a season picked on the page changes the page that is open, in the
            // spring that opens a title. Carried into the page, it would spring every view from
            // where it stood on the last season's: the titles' words sliding over each other, the
            // rows sliding past the Episodes header. The page stands still instead, and only what
            // differs changes, by its own animation: the season picked (quick), then the words,
            // episodes and facts fading through (`between`), and the art as it lands (fade-in).
            .transaction(value: id) { $0.animation = nil }
            // A title known to have no wide art, or that has some after all, moves the page to
            // its place on standard.
            .animation(.standard, value: art.isCompact)
            .background(alignment: .top) {
                // The wide art, under the clear toolbar, with the ground over its lower part,
                // where the cover and the title sit, and fading up into the art above that, where
                // a title of two lines starts. None at all where there is none. Across a season
                // switch the last season's art stays until the next one's is here, and fades to it.
                if !art.isCompact {
                    HeldHeroArt(shown: shown, layer: artLayer)
                        .frame(height: 280)
                        .clipped()
                        .overlay(alignment: .bottom) {
                            VStack(spacing: 0) {
                                BannerScrim()
                                Palette.ground.frame(height: 96)
                            }
                        }
                        .titleLanding(entry.id, .banner)
                        .playerZoomSource(entry.id)
                }
            }
        }
        .nudgeableScroll()
        .scrollIndicators(.never)
        .ignoresSafeArea(edges: .top)
        .trailerLaunchSheet(entry)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[entry.id] ?? [], in: rotor)
        .handsOff(title: entry)
        .onChange(of: laid.id, initial: true) {
            laidOut = laid.id
            if drawn == nil { drawn = laid.id }
        }
        .onChange(of: laid.id) { _, now in fadeThrough(to: now) }
        .onChange(of: kin.id, initial: true) { familyOf = kin.id }
        .onChange(of: shown.key, initial: true) {
            shownArt = shown
            artLayer += 1
        }
        .task(id: id) {
            try? await Task.sleep(for: Self.waitFor)
            waited = id
        }
        .task(id: entry.id) {
            try? await Task.sleep(for: ArtworkView.holdFor)
            artWaited = entry.id
        }
        .task(id: coming) {
            if let cover = coming.cover { store.artwork.request(cover, maxPixelSize: TitleHero.coverPixels) }
            if let wide = coming.wide { store.artwork.request(wide, maxPixelSize: HeroArtView.prefetchPixels) }
        }
        .task(id: next) {
            // The art held for is asked for now, as the page would on showing it.
            if !Self.artReady(next, in: store), let url = next.art.url {
                store.artwork.request(url, maxPixelSize: HeroArtView.prefetchPixels)
            }
        }
        .task(id: id) {
            navigator.focusedTitle = id
            // Its page, what it says about services under its buttons, and its art, asked at
            // once: the page lays out once, with all of it.
            store.prefetchPage(id)
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            store.loadDub(id)
            // What the show sets for itself, its spoiler protection among it: from the database.
            store.loadShowSettings(id)
        }
    }

    /// Fade the words, episodes and facts out on quick, draw them for `id`, and fade them in on
    /// fade-in. The swap itself is not animated: an animated one would keep the last title's
    /// parts on screen, under the next one's, for as long as it ran. A newer title laid out
    /// meanwhile is swapped to instead.
    private func fadeThrough(to id: AnilistId) {
        drawing = id
        guard !between else { return }
        withAnimation(.quick) { between = true } completion: {
            var swap = Transaction()
            swap.disablesAnimations = true
            withTransaction(swap) { drawn = drawing }
            withAnimation(.fadeIn) { between = false }
        }
    }

    /// Whether `id`'s page is here whole: its title, its episodes and what the services say
    /// (which decide its main button and the lines under it), its cover and its wide art. Each
    /// landing alone would move or swap something.
    static func pageHere(_ id: AnilistId, in store: AppStore) -> Bool {
        guard store.titles.detailed.contains(id), store.offersKnown(id) else { return false }
        let page = store.titles.page(id)
        let cover = page.coverUrl.map { store.artwork.image($0) != nil } ?? true
        return cover && artReady(ArtKey(id: id, art: store.pageArt(page)), in: store)
    }

    /// The title whose seasons and related titles the row shows for `entry`: itself once its page
    /// is here, until then `last`, the title shown before it whose page was.
    static func kin(of entry: Entry, last: AnilistId?, in store: AppStore) -> Entry {
        store.titles.detailed.contains(entry.id) ? entry : store.titles.page(last ?? entry.id)
    }

    /// Whether `key`'s art can show whole: resolved from the title itself, not a card's
    /// stand-in that knows no banner yet, and its image here.
    static func artReady(_ key: ArtKey, in store: AppStore) -> Bool {
        guard store.titles[key.id] != nil else { return false }
        switch key.art {
        case .compact: return true
        case .waiting: return false
        case .banner(let url), .still(let url), .cover(let url): return store.artwork.image(url) != nil
        }
    }
}

/// The title page's wide art: the art shown, crossfading over the last as it changes.
private struct HeldHeroArt: View {
    let shown: TitlePage.ShownArt
    let layer: Double

    var body: some View {
        ZStack {
            HeroArtView(art: shown.key.art, entry: shown.entry, holds: true, sharpCover: false)
                .id(shown.key)
                .transition(TitlePage.artSwap)
                .zIndex(layer)
        }
        .animation(.fadeIn, value: shown.key)
    }
}

/// The cover, the title block and the actions.
struct TitleHero: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    /// Whether the words are faded out, as the page changes title.
    var faded = false
    /// The title waited for long enough to show its main button without the services' word,
    /// as when it never comes offline.
    @State private var offersWaited: AnilistId?

    /// How long the main button waits for what the services say before it shows without it.
    static let offersWaitFor: Duration = .seconds(2)

    /// On an iPad the page speaks in sentence case and its actions are touch-sized (PadTitleMeta).
    #if os(iOS)
    static let pad = true
    #else
    static let pad = false
    #endif

    /// The cover's long side in pixels: 225 points at 2x.
    static let coverPixels = 460

    /// Case B: the service the main action opens.
    private var serviceMain: ServiceLink? {
        guard let availability = store.licensed.availability[entry.id], availability.case == .onService else { return nil }
        return availability.main
    }

    /// The main button: Watch on a service, Choose the match, Add a source, Play, or when it airs.
    @ViewBuilder
    private func mainAction(_ action: TitleAction) -> some View {
        if let main = serviceMain {
            // It does not play here, and it is on a service they use: Watch on it.
            Button { store.openService(entry.id, link: main, episode: nil) } label: {
                HStack(spacing: 8) {
                    Text(L.licensedWatchOn(service: main.service))
                    Image(systemName: "arrow.up.right").font(.mac(10, weight: .semibold))
                }
            }
            .buttonStyle(PrimaryCapsuleStyle(height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13))
            .keyboardShortcut(.defaultAction)
            .accessibilityHint(L.licensedOpensOutside)
        } else if action.playable, store.match.asks(entry.id) {
            ChooseMatchButton(id: entry.id, height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13)
        } else if action.playable, store.titles.unlisted(entry.id) {
            // No source has it: nothing to play, and the way to one in Play's place.
            AddSourceButton(
                sentence: Self.pad, height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13
            )
        } else if action.playable {
            Button {
                action.play(entry.id, store: store, navigator: navigator)
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: "play.fill").font(.mac(10))
                    Text(Self.pad ? action.sentence : action.title)
                }
            }
            .buttonStyle(PrimaryCapsuleStyle(height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13))
            .keyboardShortcut(.defaultAction)
        } else {
            AirsStatus(text: Self.pad ? action.sentence : action.title)
        }
    }

    var body: some View {
        let action = TitleAction(entry, handedOff: navigator.handedOff, episodes: store.titles.episodes[entry.id])
        // Add a source and Watch on a service rest on whether a source lists the title and on
        // what the services say about that: until both are known neither shows, as one would
        // only to give way to the other. Its room is kept, so nothing beside it moves.
        let offersHeld = !store.offersKnown(entry.id) && offersWaited != entry.id
            && (serviceMain != nil || (action.playable && store.titles.unlisted(entry.id) && !store.match.asks(entry.id)))
        // A title picked on the page, shown as its picker's card until it is here: its line and
        // its actions rest on what the card does not know, so their room is kept empty until
        // they are, rather than showing the card's and changing again.
        let cardOnly = store.titles[entry.id] == nil && offersWaited != entry.id
        HStack(alignment: .bottom, spacing: 28) {
            // Across a season switch the last season's cover stays until the next one's lands.
            ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: Self.coverPixels, holds: true)
                .frame(width: 150, height: 225)
                .clipShape(.rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassEdge.opacity(1.25), lineWidth: 1))
                // A poster clicked flies here, poster to poster (TitleOpening).
                .titleLanding(entry.id, .poster)
            VStack(alignment: .leading, spacing: 0) {
                // A line kept for the title and the metadata while they are on their way, so the
                // cover and the actions stand where they will.
                Text(entry.showTitle.isEmpty ? " " : entry.showTitle)
                    .font(.macShowTitle(size: 36, relativeTo: .largeTitle))
                    .macLineHeight(44, relativeTo: .largeTitle)
                    .foregroundStyle(.silk)
                    .lineLimit(2)
                    .titleOverArt()
                    .accessibilityAddTraits(.isHeader)
                OtherTitlesLine(entry: entry)
                    .padding(.top, 6)
                // Kept empty while the title is its card. When the title comes, the lines under the
                // actions can move the words up: the line fades in once they have, as text drawn
                // while it moved would be drawn at its end, over the line above.
                Text(entry.titleMetadata.isEmpty ? " " : entry.titleMetadata)
                    .groupHeaderStyle()
                    .macLineHeight(14)
                    .padding(.top, 4)
                    .animation(cardOnly ? nil : .fadeIn.delay(0.2)) { $0.opacity(cardOnly ? 0 : 1) }
                HStack(spacing: 10) {
                    mainAction(action)
                        .opacity(offersHeld ? 0 : 1)
                        .disabled(offersHeld)
                        .accessibilityHidden(offersHeld)
                        .animation(.fadeIn, value: offersHeld)
                    if entry.trailerUrl != nil {
                        Button(L.titleTrailer) {
                            navigator.playTrailer(entry, store: store)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.pad ? 48 : 28, fontSize: Self.pad ? 17 : 13))
                    }
                    if action.playable, !store.titles.unlisted(entry.id), store.tv.available {
                        Button {
                            store.playOnTV(entry.id, episode: String(action.episode), from: action.resume?.position)
                        } label: {
                            Label(L.tvPlayOn, systemImage: "tv")
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.pad ? 48 : 28, fontSize: Self.pad ? 17 : 13))
                        .help(L.titlePlayOnTvHelp)
                    }
                    #if os(iOS)
                    // The list's menu, in the header where a narrow window's bar has no room.
                    Menu {
                        StatusItems(id: entry.id)
                    } label: {
                        HStack(spacing: 8) {
                            Text(entry.listStatus.map { $0.label.capitalizedSentence } ?? L.libraryAddToList)
                            Image(systemName: "chevron.up.chevron.down").font(.ui(13, weight: .semibold))
                        }
                    }
                    .menuStyle(.button)
                    .buttonStyle(SecondaryCapsuleStyle(height: 48, fontSize: 17))
                    #endif
                }
                .padding(.top, Self.pad ? 16 : 12)
                .opacity(cardOnly ? 0 : 1)
                .disabled(cardOnly)
                .accessibilityHidden(cardOnly)
                LicensedLines(id: entry.id, font: Self.pad ? .ui(15) : .mac(13))
                MatchUncertainLine(id: entry.id, font: Self.pad ? .ui(15) : .mac(13), top: 10)
            }
            .animation(.fadeIn, value: cardOnly)
            // Another title's words are other views, drawn while the page has them faded out.
            .id(entry.id)
            .transition(.identity)
            .opacity(faded ? 0 : 1)
            .padding(.bottom, 4)
        }
        // Where it plays comes after the page, in lines under the actions: the words above glide
        // up to make room (standard) while the lines fade in, rather than jumping.
        .animation(navigator.motionReduced ? nil : .standard, value: store.licensed.availability[entry.id])
        .task(id: entry.id) {
            try? await Task.sleep(for: Self.offersWaitFor)
            offersWaited = entry.id
        }
    }
}

extension Entry {
    /// The page's metadata line: format, episodes, year, studio and score.
    var titleMetadata: String {
        var line = metadataLine
        if let score { line += (line.isEmpty ? "" : " · ") + Score.text(score) }
        return line
    }

    /// The metadata line on an iPhone's page: "TV · 24 EP · SUMMER 2026 · 79%", "18+" first for an
    /// adult title.
    var phoneMetadata: String {
        var parts: [String] = adult ? ["18+"] : []
        if let format { parts.append(TitleWords.format(format)) }
        if let episodes, episodes > 0, format != "MOVIE" { parts.append(L.titleEpisodesShort(count: Int(episodes))) }
        if let seasonWords { parts.append(seasonWords) }
        if let score { parts.append(Score.text(score)) }
        return parts.joined(separator: " · ")
    }

    /// A page's title before it has arrived, from the card another page showed for it: a
    /// season's from its franchise's picker, with the franchise's seasons, so the page has its
    /// name, cover, colour, dates and picker from the start and nothing moves as the rest lands.
    static func provisional(_ id: AnilistId, from card: LinkedTitle, seasons: [LinkedTitle]?) -> Entry {
        var entry = placeholder(id)
        entry.title = card.title
        entry.titleRomaji = card.titleRomaji
        entry.titleEnglish = card.titleEnglish
        entry.titleNative = card.titleNative
        entry.format = card.format
        entry.coverUrl = card.coverUrl
        entry.coverColor = card.coverColor
        entry.season = card.season
        entry.year = card.year
        entry.listStatus = card.listStatus
        if let seasons { entry.detail = TitleDetail(seasons: seasons) }
        return entry
    }

    /// An entry for a page whose title has not arrived yet.
    static func placeholder(_ id: AnilistId) -> Entry {
        Entry(
            id: id, title: "", secondary: nil, format: nil, episodes: nil, year: nil, score: nil,
            studio: nil, synopsis: "", coverUrl: nil, coverColor: nil, bannerUrl: nil, genres: [],
            availableOn: [], progress: nil, airingIn: nil, airingEpisode: nil, nextEpisode: nil,
            nextAiringIn: nil, lastAired: nil, resume: nil, related: [], recommended: [],
            trailerUrl: nil, listStatus: nil, titleRomaji: nil, titleEnglish: nil, titleNative: nil,
            season: nil, startDate: nil, endDate: nil, source: nil, adult: false, detail: nil, status: nil,
            synopsisSpans: []
        )
    }
}

extension TitleCache {
    /// What a title's page shows: the title as far as it is here, and before then what another
    /// page's card said of it. A season not loaded yet, or loaded from a list without its page,
    /// takes its franchise's seasons from a season that has them, so the picker stays put.
    func page(_ id: AnilistId) -> Entry {
        var family: (card: LinkedTitle, seasons: [LinkedTitle])?
        var related: LinkedTitle?
        if self[id]?.detail == nil {
            for known in entries.values {
                guard let detail = known.detail else { continue }
                if let seasons = detail.seasons, let card = seasons.first(where: { $0.id == id }) {
                    family = (card, seasons)
                    break
                }
                if related == nil { related = detail.relations.first { $0.id == id } }
            }
        }
        if var entry = self[id] {
            if entry.detail == nil, let family { entry.detail = TitleDetail(seasons: family.seasons) }
            return entry
        }
        if let family { return .provisional(id, from: family.card, seasons: family.seasons) }
        if let related { return .provisional(id, from: related, seasons: nil) }
        return .placeholder(id)
    }
}

extension AppStore {
    /// Ask for what `id`'s page needs before it opens: its page, and its cover and wide art at
    /// the sizes the page shows them.
    func prefetchPage(_ id: AnilistId) {
        if !titles.detailed.contains(id) { loadDetail(id) }
        if licensed.availability[id] == nil { loadAvailability(id, playsHere: !titles.unlisted(id)) }
        let entry = titles.page(id)
        if let cover = entry.coverUrl { artwork.request(cover, maxPixelSize: TitleHero.coverPixels) }
        if let banner = entry.bannerUrl { artwork.request(banner, maxPixelSize: HeroArtView.prefetchPixels) }
    }
}

/// Seasons, then Related: each a cover with its name and where it stands.
struct TitleFamilyRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let family: TitleFamily
    /// Whether Related is faded out, as the page changes title. It is the last title's until
    /// then, so it fades through the ground with the words and episodes rather than going at
    /// once; the seasons stay, their pick moving on the click.
    var faded = false

    /// A related card's cover: 36 by 54 on the Mac, 40 by 60 on iPad.
    static var cover: CGSize {
        #if os(macOS)
        CGSize(width: 36, height: 54)
        #else
        CGSize(width: 40, height: 60)
        #endif
    }

    /// How wide Related keeps itself beside the seasons: a card and part of the next, cut by the
    /// edge to say the row scrolls.
    static let relatedRoom: CGFloat = 240

    var body: some View {
        // Side by side, as MacTitle and PadTitle have them, while the seasons fit with room for
        // Related beside them. A long franchise's seasons scroll sideways on a line of their own
        // instead, the season on screen in view, with Related under them.
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 32) {
                if !family.seasons.isEmpty {
                    seasonsGroup {
                        HStack(spacing: 8) {
                            ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                                seasonCell(index, season)
                            }
                        }
                        // Each cell at its own width, so a tight row fails to fit and the
                        // scrolling layout takes over instead of the cells squeezing.
                        .fixedSize(horizontal: true, vertical: false)
                    }
                }
                if hasRelated {
                    fadingRelated.frame(minWidth: Self.relatedRoom, idealWidth: Self.relatedRoom, maxWidth: .infinity, alignment: .leading)
                }
            }
            VStack(alignment: .leading, spacing: 20) {
                if !family.seasons.isEmpty {
                    seasonsGroup { SeasonScroller(family: family) { index, season in seasonCell(index, season) } }
                }
                fadingRelated
            }
        }
    }

    /// Whether there is anything under Related.
    private var hasRelated: Bool {
        entry.detail != nil ? !entry.relatedCards(excluding: family.picked).isEmpty : !family.related.isEmpty
    }

    private func seasonsGroup(@ViewBuilder _ row: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L.titleSeasons).groupHeaderStyle().padding(.horizontal, 4)
                .accessibilityAddTraits(.isHeader)
            row()
        }
    }

    private func seasonCell(_ index: Int, _ season: TitleFamily.Season) -> some View {
        familyCell(
            id: season.id,
            name: L.seasonNumber(number: index + 1),
            detail: seasonDetail(season),
            counts: season.id == entry.id && entry.progress?.done != nil && entry.progressOf != nil,
            fraction: season.id == entry.id ? entry.watchedFraction : nil,
            selected: season.isThis
        )
    }

    private var fadingRelated: some View {
        related.opacity(faded ? 0 : 1)
    }

    @ViewBuilder private var related: some View {
        if entry.detail != nil {
            let cards = entry.relatedCards(excluding: family.picked)
            if !cards.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text(L.titleRelated).groupHeaderStyle().padding(.horizontal, 4)
                        .accessibilityAddTraits(.isHeader)
                    // A row that scrolls sideways, its last card cut by the edge to say so.
                    ScrollView(.horizontal) {
                        HStack(spacing: 8) {
                            ForEach(cards, id: \.id) { linked in
                                RelatedCard(linked: linked, cover: Self.cover)
                                    .frame(height: 62)
                            }
                        }
                    }
                    .scrollIndicators(.never)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        } else if !family.related.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text(L.titleRelated).groupHeaderStyle().padding(.horizontal, 4)
                    .accessibilityAddTraits(.isHeader)
                HStack(spacing: 8) {
                    ForEach(family.related, id: \.id) { related in
                        familyCell(
                            id: related.id,
                            name: related.title,
                            detail: [related.format.map(TitleWords.format), related.relation.capitalizedSentence]
                                .compactMap { $0 }.joined(separator: " · "),
                            fraction: nil,
                            selected: false
                        )
                    }
                }
            }
        }
    }

    private func seasonDetail(_ season: TitleFamily.Season) -> String {
        if season.id == entry.id, let done = entry.progress?.done, let of = entry.progressOf {
            return "\(done) / \(of.total)"
        }
        if let known = store.titles[season.id], let words = known.seasonWords { return words }
        // The page's own card dates a season the app has not loaded: "Spring 2027".
        if let linked = card(season.id), let year = linked.year {
            return linked.season.map { L.seasonWithYear(season: TitleWords.season($0), year: String(year)) } ?? String(year)
        }
        return season.isThis ? L.continueThisSeason : L.formatTv
    }

    private func familyCell(
        id: AnilistId, name: String, detail: String, counts: Bool = false, fraction: Double?, selected: Bool
    ) -> some View {
        Button {
            if !selected { navigator.openTitle(id) }
        } label: {
            HStack(spacing: 10) {
                let known = store.titles[id]
                let linked = card(id)
                // The card's cover stays while the page's own is on its way, as a season's page
                // comes in: never the plate between them.
                ArtworkView(url: known?.coverUrl ?? linked?.coverUrl, color: known?.plateColor, maxPixelSize: 108, holds: true)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: 6))
                VStack(alignment: .leading, spacing: 0) {
                    Text(name)
                        .font(.mac(13, weight: .semibold))
                        .macLineHeight(18)
                        .foregroundStyle(selected ? .amber : .silk)
                        .lineLimit(1)
                    Text(detail)
                        .font(.mac(12))
                        .macLineHeight(16)
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        // "6 / 12" to "7 / 12", the changed digit rolling (T9). A count to a
                        // date, as the selection leaves a season, is another line: it comes in
                        // on fade-in where the last one went at once, never over it.
                        .contentTransition(.numericText())
                        .animation(.standard, value: detail)
                        .id(counts)
                        .transition(.asymmetric(insertion: .opacity.animation(.fadeIn), removal: .identity))
                    if let fraction {
                        Meter(fraction: fraction, height: 3).frame(width: 112).padding(.top, 6)
                    }
                }
                .frame(minWidth: 72, alignment: .leading)
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 4, trailing: 14))
            .frame(height: 62)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
            // A season picked: the selection moves to it, and nothing else in the row does.
            .animation(.quick, value: selected)
        }
        .buttonStyle(.plain)
        .pointerHover()
        // Pointed at, it is likely next: its page and its art asked for now, so the page opens
        // whole.
        .onHover { inside in if inside, !selected { store.prefetchPage(id) } }
        // The cover, where the page's seasons and relations do not carry it.
        .task { if store.titles[id] == nil, card(id) == nil { store.loadDetail(id) } }
    }

    /// The page's own card for `id`: a season's, which every season has, or a relation's.
    private func card(_ id: AnilistId) -> LinkedTitle? {
        entry.detail?.seasons?.first { $0.id == id } ?? entry.detail?.relations.first { $0.id == id }
    }
}

/// A long franchise's seasons in a row that scrolls sideways, its last cell cut by the edge to say
/// so: lazy, so only the seasons in view are made and only their covers asked for, and the season
/// on screen in the middle as the page opens and as it changes.
struct SeasonScroller<Cell: View>: View {
    let family: TitleFamily
    @ViewBuilder let cell: (Int, TitleFamily.Season) -> Cell

    var body: some View {
        let chosen = family.seasons.first(where: \.isThis)?.id
        ScrollViewReader { proxy in
            ScrollView(.horizontal) {
                LazyHStack(spacing: 8) {
                    ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                        cell(index, season).id(season.id)
                    }
                }
            }
            .scrollIndicators(.never)
            .onAppear { if let chosen { proxy.scrollTo(chosen, anchor: .center) } }
            .onChange(of: chosen) { _, now in
                if let now { withAnimation(.standard) { proxy.scrollTo(now, anchor: .center) } }
            }
        }
    }
}

/// The episodes: a row each, the next one marked, stills of what comes after it blurred.
struct EpisodeList: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var unwatchedOnly = false
    /// The last title whose rows were on screen, and how tall they stood: another title's
    /// placeholders keep that room, so nothing below or beside them moves until its rows come.
    @State private var lastRows: (id: AnilistId, height: CGFloat)?
    let entry: Entry
    /// Where the page's Unwatched episodes rotor finds the rows.
    let rotor: Namespace.ID

    var body: some View {
        let rows = episodeRows
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text(L.titleEpisodes)
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                // How far the dub goes, when the engine has said: faded in, where it moves nothing.
                // Another title's line is another view, gone at once rather than faded out.
                let dub = store.titles.dubLines[entry.id]
                ZStack(alignment: .leading) {
                    if let dub {
                        Text(dub)
                            .font(.mac(13))
                            .foregroundStyle(.haze)
                            .lineLimit(1)
                            .transition(.opacity)
                    }
                }
                .animation(.fadeIn, value: dub)
                .id(entry.id)
                .transition(.identity)
                Spacer()
                SegmentedCapsule(selection: $unwatchedOnly, options: [(false, L.titleEpisodesAll), (true, L.titleEpisodesUnwatched)])
            }
            .frame(height: 28)
            .padding(.bottom, 6)
            let loaded = store.titles.episodes[entry.id] != nil
            // The placeholders and the rows in one place, one crossfading into the other (T8):
            // none slides or comes in alone.
            ZStack(alignment: .topLeading) {
                if loaded {
                    VStack(alignment: .leading, spacing: 0) {
                        if rows.isEmpty {
                            QuietLine(unwatchedOnly ? L.titleAllWatched : L.titleNoEpisodes)
                        }
                        // The watched run folds away above the last episode watched; Unwatched
                        // has none.
                        let fold = unwatchedOnly ? nil : WatchedFold(rows)
                        if let fold {
                            WatchedFoldSection(
                                fold: fold, shown: navigator.watchedShown(entry.id), phone: false,
                                toggle: { navigator.toggleWatched(entry.id) }
                            ) {
                                ForEach(rows.prefix(fold.count), id: \.number) { row($0) }
                            }
                        }
                        ForEach(rows.dropFirst(fold?.count ?? 0), id: \.number) { row($0) }
                    }
                    .transition(.opacity)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { lastRows = (entry.id, $0) }
                } else {
                    EpisodePlaceholders(count: entry.episodes)
                        .frame(minHeight: lastRows.flatMap { $0.id == entry.id ? nil : $0.height }, alignment: .top)
                        .transition(.opacity)
                }
            }
            .animation(.standard, value: loaded)
            // Another title's rows are other views, drawn while the page has them faded out.
            .id(entry.id)
            .transition(.identity)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func row(_ row: EpisodeRow) -> some View {
        EpisodeRowView(entry: entry, row: row, spoilers: store.spoilerProtection(for: entry.id))
            .accessibilityRotorEntry(id: row.number, in: rotor)
    }

    /// The table, dated by the schedule, with the next episode still to air at its end.
    private var episodeRows: [EpisodeRow] {
        guard let loaded = store.titles.episodes[entry.id] else { return [] }
        let rows = entry.episodeTable(loaded)
        return unwatchedOnly ? rows.filter { !$0.completed } : rows
    }
}

/// One episode in the list.
struct EpisodeRowView: View {
    @Environment(AppStore.self) private var store
    let entry: Entry
    let row: EpisodeRow
    let spoilers: Bool

    var body: some View {
        if row.isUnaired(), let date = row.airsDate {
            UnairedEpisodeRow(row: row, airs: date)
        } else {
            VStack(spacing: 0) {
                airedRow
                    #if os(macOS)
                    .help(row.airsDate.map(AirsWords.aired) ?? "")
                    #endif
                DiscussionLine(entry: entry, row: row, inset: 140)
            }
        }
    }

    @ViewBuilder
    private var airedRow: some View {
        let number = UInt32(row.number) ?? 0
        let current = number == (entry.progress?.next ?? 1)
        // One no source has: nothing to play, so a click offers what can be done, adding a
        // source among it, rather than playing.
        if let link = store.licensed.episodeLink(entry.id, number) {
            // On a service they use: the row opens the service, and says so with the glyph.
            Button {
                store.openService(entry.id, link: ServiceLink(service: link.service, url: link.url, notes: nil), episode: number)
            } label: {
                rowLabel(number: number, current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .accessibilityHint(L.licensedOpensOutside)
        } else if row.metadataOnly || row.unsourced {
            Menu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } label: {
                rowLabel(number: number, current: current)
            }
            .menuStyle(.button)
            .menuIndicator(.hidden)
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
        } else {
            Button {
                store.send(.play(id: entry.id, episode: row.number))
            } label: {
                rowLabel(number: number, current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
        }
    }

    private func rowLabel(number: UInt32, current: Bool) -> some View {
        let next = entry.progress?.next ?? 1
        return HStack(spacing: 14) {
            Text(row.number)
                .font(.mac(13, weight: current ? .semibold : .regular))
                .monospacedDigit()
                .foregroundStyle(current ? .amber : .haze)
                .animation(.quick, value: current)
                .frame(width: 22, alignment: .trailing)
            EpisodeStill(
                entry: entry, row: row,
                hidden: spoilers && number > next && !row.completed,
                unaired: false
            )
            .opacity(row.completed ? 0.7 : 1)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(row.title ?? L.episodeNumberString(number: row.number))
                        .foregroundStyle(row.completed ? .haze : .silk)
                        .lineLimit(1)
                        .animation(.quick, value: row.completed)
                    if row.kind == .filler {
                        Text(L.episodeFiller).groupHeaderStyle()
                    }
                }
                if row.unsourced {
                    Text(L.episodeUnsourced).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                }
                if current, let fraction = entry.resume?.fraction {
                    Held(value: fraction) { Meter(fraction: $0, height: 3).frame(width: 200) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing(current: current)
            WatchedCheck(watched: row.completed, size: 11)
                .frame(width: 14)
        }
        .font(.mac(13))
        .opacity(row.unsourced && store.licensed.episodeLink(entry.id, number) == nil ? 0.55 : 1)
        .padding(.horizontal, 8)
        .frame(height: 44)
        .background(current ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
    }

    @ViewBuilder
    private func trailing(current: Bool) -> some View {
        if store.licensed.episodeLink(entry.id, UInt32(row.number) ?? 0) != nil {
            Image(systemName: "arrow.up.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
        } else if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
            let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
            Held(value: left) { MinutesLeft(minutes: $0) }
        } else if let seconds = row.durationSecs {
            Text(L.timeMinutes(minutes: max(1, Int((Double(seconds) / 60).rounded())))).foregroundStyle(.haze)
        }
    }
}

/// The next episode still to air: no button, since nothing can play it; the flat placeholder for
/// its still, its title in haze, and when it airs in water ("Airs Thursday 16:30").
struct UnairedEpisodeRow: View {
    let row: EpisodeRow
    let airs: Date

    var body: some View {
        let title = row.title ?? L.episodeNumberString(number: row.number)
        let when = AirsWords.airs(airs)
        HStack(spacing: 14) {
            Text(row.number)
                .monospacedDigit()
                .foregroundStyle(.haze)
                .frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(Palette.artPlaceholder).frame(width: 64, height: 36)
            Text(title).foregroundStyle(.haze).lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 6) {
                Circle().fill(.water).frame(width: 6, height: 6)
                Text(when).monospacedDigit()
            }
            .foregroundStyle(.water)
            Color.clear.frame(width: 14)
        }
        .font(.mac(13))
        .padding(.horizontal, 8)
        .frame(height: 44)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.titleEpisodeRowUnaired(title: row.title.map { L.episodeTitledText(number: row.number, title: $0) } ?? L.episodeNumberString(number: row.number), when: when))
    }
}

/// An episode's still: its art, blurred with an eye when spoiler protection hides it, and a plain
/// plate before it airs.
struct EpisodeStill: View {
    let entry: Entry
    let row: EpisodeRow
    let hidden: Bool
    let unaired: Bool
    var size = CGSize(width: 64, height: 36)

    var body: some View {
        Group {
            if unaired {
                Rectangle().fill(CoverPlate.muted(entry.plateColor))
            } else if hidden {
                ZStack {
                    ArtworkView(url: row.thumbnail ?? entry.bannerUrl, color: entry.plateColor, maxPixelSize: 128)
                        .blur(radius: 4)
                        .scaleEffect(1.15)
                    Image(systemName: "eye.slash")
                        .font(.mac(12, weight: .medium))
                        .foregroundStyle(.silk)
                }
                .accessibilityLabel(L.episodeSpoilerHidden)
            } else {
                ArtworkView(url: row.thumbnail ?? entry.bannerUrl, color: entry.plateColor, maxPixelSize: 128)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipShape(.rect(cornerRadius: 6))
    }
}

/// The episode rows while the table loads: as many as the title has, where that is known, up to
/// a screenful, so the rows that replace them take the room they kept.
struct EpisodePlaceholders: View {
    let count: UInt32?

    var body: some View {
        VStack(spacing: 0) {
            ForEach(0..<Int(min(max(count ?? 4, 1), 12)), id: \.self) { _ in EpisodePlaceholderRow() }
        }
    }
}

/// An episode row while the table loads.
struct EpisodePlaceholderRow: View {
    var body: some View {
        HStack(spacing: 14) {
            Text("7").frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 64, height: 36)
            Text("The Lantern Road") // i18n-skip
            Spacer()
            Text("24 min") // i18n-skip
        }
        .font(.mac(13))
        .foregroundStyle(.haze)
        .redacted(reason: .placeholder)
        .padding(.horizontal, 8)
        .frame(height: 44)
        .accessibilityHidden(true)
    }
}

/// The synopsis, the facts and the sheet's opener beside the episodes: eight lines on the
/// Mac, three lines on iPad with "More" to show the rest in place.
struct TitleDetails: View {
    let entry: Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let synopsis = entry.shownSynopsis
            if !synopsis.isEmpty {
                #if os(macOS)
                InlineMoreText(text: synopsis, lines: 8, font: .mac(13), lineHeight: 20)
                    .id(entry.id)
                #else
                InlineMoreText(text: synopsis, lines: 3, font: .mac(13))
                    .id(entry.id)
                #endif
            }
            if entry.isDemo { DemoCredit() }
            MetaFactsGrid(entry: entry)
            MoreAboutButton(entry: entry)
                .padding(.top, 4)
        }
        .padding(.top, 4)
    }
}

/// The title page's toolbar on the Mac: back, its list, and More.
struct TitleToolbar: ToolbarContent {
    let store: AppStore
    let navigator: Navigator
    let id: AnilistId

    var body: some ToolbarContent {
        ToolbarItem(placement: .navigation) {
            Button(L.actionBack, systemImage: "chevron.left") {
                withAnimation(.standard) { navigator.back() }
            }
            .keyboardShortcut("[", modifiers: .command)
        }
        ToolbarItem(placement: .primaryAction) {
            Menu {
                StatusItems(id: id)
            } label: {
                Text(store.titles[id]?.listStatus.map { $0.label.capitalizedSentence } ?? L.libraryAddToList)
            }
            .fixedSize()
        }
        ToolbarItem(placement: .primaryAction) {
            Menu {
                if let entry = store.titles[id] {
                    TitleMoreItems(entry: entry)
                }
            } label: {
                Label(L.actionMore, systemImage: "ellipsis")
            }
        }
    }
}
