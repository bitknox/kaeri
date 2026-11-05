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
    @Environment(\.titleAwaited) private var awaited
    @Namespace private var rotor
    let id: AnilistId
    /// The title the page is laid out for: the last one opened, until the next one's is here.
    @State private var laidOut: AnilistId?
    /// The title that has been waited for long enough, laid out with what is known of it.
    @State private var waited: AnilistId?
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
    static let waitFor: Duration = .milliseconds(400)

    var body: some View {
        // A season picked on the page shows when its page is here, in one change, rather
        // than first as the card the picker had for it and again as itself, everything that
        // differs between the two moving twice. Only the selection moves at once. Past
        // `waitFor` it shows what is known of it. Either way the page dissolves from the last
        // title to the next in place: each part crossfades by its own animation, with nothing
        // emptied in between.
        let ready = laidOut == nil || laidOut == id || waited == id || Self.pageHere(id, in: store)
        let entry = store.titles.page(ready ? id : laidOut ?? id)
        // A season's stand-in, from its picker's card, knows no banner, and a cover or a banner
        // on its way would show as a plate first: the last art stays until the next is whole, or
        // until it is known there is none. With no banner and no still seen, the cover fills the
        // band in its own colours, behind the header, as on every Mac screen with a hero.
        let next = ArtKey(id: entry.id, art: store.heroArt(entry))
        let shown = shownArt.flatMap { held in
            held.key != next && !Self.artReady(next, in: store) && (artWaited != next.id || next.art == .waiting) ? held : nil
        } ?? ShownArt(key: next, entry: entry)
        let art = shown.key.art
        // A related title shown as its card knows no seasons and no related titles of its own:
        // the row it was picked from stays until its page says what is in it.
        let kin = Self.kin(of: entry, last: familyOf, in: store)
        // What the title picked shows with, asked for while the last one's page stays.
        let picked = ready ? nil : store.titles.page(id)
        let coming = Coming(cover: picked?.coverUrl, wide: picked.flatMap { store.heroArt($0).url })
        ScrollView {
            // Inside the scroll view, which the reader's proxy finds as the one holding the
            // views it scrolls to; the scroll view's own modifiers stay on it.
            ScrollViewReader { scroller in
                VStack(alignment: .leading, spacing: 0) {
                    // With no wide art the page starts higher: under the toolbar, not under a banner.
                    Color.clear.frame(height: art.isCompact ? 64 : 96)
                    // Art flying onto the banner covers the cover and the words over it: they come in
                    // once it has landed, rather than at once with the art drawn under them.
                    let underFlyer = awaited == TitleLandingID(id: entry.id, art: .banner)
                    TitleHero(entry: entry)
                        .opacity(underFlyer ? 0 : 1)
                        .animation(.fadeIn, value: underFlyer)
                    let family = TitleFamily(kin, this: id)
                    if !family.seasons.isEmpty || !family.related.isEmpty {
                        TitleFamilyRow(entry: kin, family: family)
                            .padding(.top, 20)
                    }
                    HStack(alignment: .top, spacing: 32) {
                        EpisodeList(entry: entry, rotor: rotor, scroller: scroller)
                        TitleDetails(entry: entry)
                            .frame(width: 260)
                    }
                    .padding(.top, 24)
                    // On the ground, so Related cards dissolving as the episodes move up go under
                    // them rather than through their words.
                    .background(.ground)
                }
                .padding(EdgeInsets(top: 0, leading: 24, bottom: 20, trailing: 28))
                // On the Mac a season picked on the page changes the page that is open, in the
                // spring that opens a title. Carried into the page, it would spring every view from
                // where it stood on the last season's: the titles' words sliding over each other, the
                // rows sliding past the Episodes header. The page stands still instead, and only what
                // differs changes, by its own animation: the season picked (quick), the hero's words
                // (quick), the episodes as one block (standard), and the art as it lands (fade-in).
                .transaction(value: id) { $0.animation = nil }
                // A title picked from Related, shown as its card, keeps the last page's row until its
                // own page comes. When the row changes then, or with a season's header, the episodes
                // move to their place with it as its cards dissolve, rather than jumping under them.
                .animation(.quick, value: kin.id)
                // With Reduce Motion a season picked changes the page at once: the selection, the
                // words, the episodes and Related, each without its own animation.
                .transaction(value: [id, entry.id, kin.id]) { if navigator.motionReduced { $0.disablesAnimations = true } }
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
        }
        #if os(macOS)
        .overlay(alignment: .topTrailing) { TypedCapsuleSlot(title: entry.id, top: 64) }
        #endif
        .nudgeableScroll()
        .scrollIndicators(.never)
        .ignoresSafeArea(edges: .top)
        #if os(iOS)
        // Under the navigation bar, which the page's art runs beneath.
        .overlay(alignment: .topTrailing) { TypedCapsuleSlot(title: entry.id, top: 12) }
        #endif
        .trailerLaunchSheet(entry)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[entry.id] ?? [], in: rotor)
        .handsOff(title: entry)
        .onChange(of: entry.id, initial: true) { laidOut = entry.id }
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
        .task(id: Self.seasonArt(around: id, in: TitleFamily(kin, this: id), store: store)) {
            // The seasons' headers came as cards with this page: their art is asked now, as the
            // page would on showing them, so a season picked lands whole.
            for art in Self.seasonArt(around: id, in: TitleFamily(kin, this: id), store: store) {
                store.artwork.request(art.url, maxPixelSize: art.pixels)
            }
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
            store.loadEpisodesForPage(id)
            store.loadDub(id)
            // What the show sets for itself, its spoiler protection among it: from the database.
            store.loadShowSettings(id)
        }
    }

    /// Whether `id`'s page is here: its title and its episodes, which lay out the words, the facts
    /// and the rows. Its cover and its wide art crossfade in when they land, the last ones held
    /// until then, and its main button fades in once the services have said.
    ///
    /// A season whose header the engine kept is here at once: its words, facts, synopsis and art
    /// are the page's own, so the page crossfades to it on the click, and its rows crossfade in
    /// from their placeholders when the table comes.
    static func pageHere(_ id: AnilistId, in store: AppStore) -> Bool {
        store.titles.detailed.contains(id) && store.titles.episodes[id] != nil
            || store.titles.headed.contains(id)
    }

    /// How many seasons either side of the one shown have their art decoded ahead, at the sizes
    /// the page shows them, so a season picked near it has its banner and cover whole on the
    /// click. Farther ones are asked when the pointer rests on them.
    static let seasonArtAhead = 3

    /// One picture asked ahead, at the size the page shows it.
    struct ArtAhead: Hashable {
        let url: String
        let pixels: Int
    }

    /// The covers and wide art of the seasons around `id` in `family`, as the page would show
    /// them, for the ones whose header is known: the nearest first, as a pick is likeliest there.
    static func seasonArt(around id: AnilistId, in family: TitleFamily, store: AppStore) -> [ArtAhead] {
        guard let at = family.seasons.firstIndex(where: { $0.id == id }) else { return [] }
        let near = family.seasons.indices
            .filter { $0 != at && abs($0 - at) <= seasonArtAhead }
            .sorted { abs($0 - at) < abs($1 - at) }
        return near.flatMap { index -> [ArtAhead] in
            guard let known = store.titles[family.seasons[index].id] else { return [] }
            return [
                known.coverUrl.map { ArtAhead(url: $0, pixels: TitleHero.coverPixels) },
                store.heroArt(known).url.map { ArtAhead(url: $0, pixels: HeroArtView.prefetchPixels) },
            ].compactMap { $0 }
        }
    }

    /// The title whose seasons and related titles the row shows for `entry`: itself once its page
    /// or its kept header is here, until then `last`, the title shown before it whose page was.
    /// A header names its seasons and its relations, so the row takes its place with the header.
    static func kin(of entry: Entry, last: AnilistId?, in store: AppStore) -> Entry {
        store.titles.detailed.contains(entry.id) || store.titles.headed.contains(entry.id)
            ? entry : store.titles.page(last ?? entry.id)
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
    /// The title waited for long enough to show its main button without the services' word,
    /// as when it never comes offline.
    @State private var offersWaited: AnilistId?
    /// Whether Play on TV was showing, kept while the next season's episodes are on their way.
    @State private var tvShown = false
    /// The service the main button last named, kept while the next season's offers are on
    /// their way, so a season on the same service keeps its button rather than blinking.
    @State private var heldMain: ServiceLink?

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
    private func mainAction(_ action: TitleAction, held: ServiceLink?) -> some View {
        if let main = serviceMain ?? held {
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
        let offersPending = !store.offersKnown(entry.id) && offersWaited != entry.id
        // A season switched to from one shown on a service keeps that button until its own
        // offers say otherwise.
        let held = serviceMain == nil && offersPending ? heldMain : nil
        let offersHeld = offersPending && heldMain == nil
            && (serviceMain != nil || (action.playable && store.titles.unlisted(entry.id) && !store.match.asks(entry.id)))
        // Whether a source lists the title is known once its episodes are. Until then Play on TV
        // stays as it was, so a season shown from its header keeps the button it will have, or
        // goes without it, rather than changing twice.
        let playsOnTV = action.playable && store.tv.available
            && (store.titles.episodes[entry.id] != nil ? !store.titles.unlisted(entry.id) : tvShown)
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
                Text(entry.titleMetadata.isEmpty ? " " : entry.titleMetadata)
                    .groupHeaderStyle()
                    .macLineHeight(14)
                    .padding(.top, 4)
                HStack(spacing: 10) {
                    mainAction(action, held: held)
                        .opacity(offersHeld ? 0 : 1)
                        .disabled(offersHeld)
                        .allowsHitTesting(held == nil)
                        .accessibilityHidden(offersHeld)
                        .animation(.fadeIn, value: offersHeld)
                    if entry.trailerUrl != nil {
                        Button(L.titleTrailer) {
                            navigator.playTrailer(entry, store: store)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.pad ? 48 : 28, fontSize: Self.pad ? 17 : 13))
                    }
                    if playsOnTV {
                        Button {
                            store.playOnTV(entry.id, episode: String(action.episode), from: action.resume?.position)
                        } label: {
                            Label(L.tvPlayOn, systemImage: "tv")
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.pad ? 48 : 28, fontSize: Self.pad ? 17 : 13))
                        .tooltip(L.titlePlayOnTvHelp)
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
                .animation(.fadeIn, value: playsOnTV)
                .padding(.top, Self.pad ? 16 : 12)
                LicensedLines(id: entry.id, font: Self.pad ? .ui(15) : .mac(13))
                MatchUncertainLine(id: entry.id, font: Self.pad ? .ui(15) : .mac(13), top: 10)
            }
            // Another season's words swap in place: one short fade.
            .contentTransition(.opacity)
            .animation(.quick, value: entry.id)
            .padding(.bottom, 4)
        }
        // Where it plays comes after the page, in lines under the actions: the words above glide
        // up to make room (standard) while the lines fade in, rather than jumping.
        .animation(navigator.motionReduced ? nil : .standard, value: store.licensed.availability[entry.id])
        .onChange(of: playsOnTV, initial: true) { tvShown = playsOnTV }
        .onChange(of: serviceMain, initial: true) { if let serviceMain { heldMain = serviceMain } }
        .onChange(of: offersPending) { if !offersPending, serviceMain == nil { heldMain = nil } }
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
        prefetchArt(id)
    }

    /// Ask for `id`'s cover and wide art at the sizes its page shows them, and nothing of
    /// AniList: for a season cell under the pointer. The season's header is kept with its
    /// franchise once the page is shown, so its page opens with its top at once and asks for
    /// the rest when it opens.
    func prefetchArt(_ id: AnilistId) {
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
                    related.frame(minWidth: Self.relatedRoom, idealWidth: Self.relatedRoom, maxWidth: .infinity, alignment: .leading)
                }
            }
            VStack(alignment: .leading, spacing: 20) {
                if !family.seasons.isEmpty {
                    seasonsGroup { SeasonScroller(family: family) { index, season in seasonCell(index, season) } }
                }
                related
            }
        }
        // A season picked: the selection moves to it, the two cells taking their new widths
        // together and the row's layout with them. Another title's Related cards dissolve into
        // the last one's rather than changing at once.
        .animation(.quick, value: family.seasons.first(where: \.isThis)?.id)
        .animation(.quick, value: entry.id)
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
            // The meter goes with the selection, so it moves on the click with the box and the
            // words, rather than once the season's page has come.
            fraction: season.isThis ? store.titles.page(season.id).watchedFraction : nil,
            selected: season.isThis
        )
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
                .id(entry.id)
                .transition(.opacity)
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
            .id(entry.id)
            .transition(.opacity)
        }
    }

    private func seasonDetail(_ season: TitleFamily.Season) -> String {
        // The count goes with the selection too.
        if season.isThis {
            let own = store.titles.page(season.id)
            if let done = own.progress?.done, let of = own.progressOf { return "\(done) / \(of.total)" }
        }
        if let known = store.titles[season.id], let words = known.seasonWords { return words }
        // The page's own card dates a season the app has not loaded: "Spring 2027".
        if let linked = card(season.id), let year = linked.year {
            return linked.season.map { L.seasonWithYear(season: TitleWords.season($0), year: String(year)) } ?? String(year)
        }
        return season.isThis ? L.continueThisSeason : L.formatTv
    }

    private func familyCell(
        id: AnilistId, name: String, detail: String, fraction: Double?, selected: Bool
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
                        // "6 / 12" to "7 / 12", the changed digit rolling (T9).
                        .contentTransition(.numericText())
                        .animation(.standard, value: detail)
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
        .onHover { inside in if inside, !selected { store.prefetchArt(id) } }
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
    /// The iPad's Reduce Motion; the Mac's reaches `navigator.motionReduced` through its shell.
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var unwatchedOnly = false
    /// The last title whose rows were on screen, and how tall they stood: another title's
    /// placeholders keep that room, so nothing below or beside them moves until its rows come.
    @State private var lastRows: (id: AnilistId, height: CGFloat)?
    /// Whether the rows keep room for text, settled when the table is laid out.
    @State private var room = EpisodeRoom()
    /// How wide the list stands, which sets whether text sits beside the stills or under them.
    @State private var listWidth: CGFloat?
    /// Where the pickers stand from the top of the page's visible area. Kept out of the view's
    /// state, so scrolling redraws nothing.
    @State private var pickers = PickersPlace()
    /// Whether Go to episode's popover is open, hanging from the range menu or the heading.
    @State private var goToOpen = false
    let entry: Entry
    /// Where the page's Unwatched episodes rotor finds the rows.
    let rotor: Namespace.ID
    /// The page's scroll view, which a range switch brings the pickers back into.
    var scroller: ScrollViewProxy?

    /// The id of the line that holds the pickers, which a range switch scrolls to.
    static let pickers = "episode-pickers"
    /// Above this, from the top of the visible area, the pickers are under the toolbar or
    /// scrolled away.
    private static let pickersHidden: CGFloat = 64

    /// The rows' sizes: the Mac's by the list's width, the iPad's own.
    private var metrics: EpisodeMetrics {
        #if os(macOS)
        .mac(listWidth: listWidth ?? EpisodeMetrics.wideFrom)
        #else
        .pad
        #endif
    }

    var body: some View {
        let rows = episodeRows
        let plan = EpisodePlan(rows: rows, picked: navigator.episodeRange[entry.id])
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text(L.titleEpisodes)
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                    .goToPopover(isPresented: plan.ranges.isEmpty ? $goToOpen : .constant(false), rows: rows) { land($0, rows: rows, plan: plan) }
                // How far the dub goes, when the engine has said: faded in, where it moves nothing.
                if let dub = store.titles.dubLines[entry.id] {
                    Text(dub)
                        .font(.mac(13))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        .transition(.opacity.animation(.fadeIn))
                }
                // A long series' ranges of 100, beside the dub line; View › Previous Episodes and
                // Next Episodes step through them.
                if !plan.ranges.isEmpty {
                    EpisodeRangeMenu(plan: plan, rows: rows, phone: false, goTo: { goToOpen = true }) {
                        navigator.episodeRange[entry.id] = $0
                    }
                    .goToPopover(isPresented: $goToOpen, rows: rows) { land($0, rows: rows, plan: plan) }
                }
                Spacer()
                SegmentedCapsule(selection: $unwatchedOnly, options: [(false, L.titleEpisodesAll), (true, L.titleEpisodesUnwatched)])
            }
            .frame(height: 28)
            .padding(.bottom, 6)
            .animation(.fadeIn, value: store.titles.dubLines[entry.id])
            .id(Self.pickers)
            .onGeometryChange(for: CGFloat.self) { $0.frame(in: .scrollView).minY } action: { [pickers] top in pickers.top = top }
            let loaded = store.titles.episodes[entry.id] != nil
            // The placeholders and the rows in one place, one crossfading into the other (T8), and
            // another season's rows into this one's as one block: none slides or comes in alone.
            ZStack(alignment: .topLeading) {
                Group {
                    if loaded {
                        VStack(alignment: .leading, spacing: 0) {
                            if rows.isEmpty {
                                QuietLine(unwatchedOnly ? L.titleAllWatched : L.titleNoEpisodes)
                            }
                            // The watched run folds away above the last episode watched; Unwatched
                            // has none. A long series shows one range of a hundred, its watched run
                            // folded the same way.
                            let fold = unwatchedOnly && plan.ranges.isEmpty ? nil : plan.fold
                            let spoilers = store.spoilerProtection(for: entry.id)
                            let protectedFrom = EpisodeProtection.start(plan.shown, spoilers: spoilers)
                            if let fold {
                                WatchedFoldSection(
                                    fold: fold, shown: navigator.watchedShown(entry.id), phone: false,
                                    toggle: { navigator.toggleWatched(entry.id) }
                                ) {
                                    ForEach(plan.shown.prefix(fold.count), id: \.number) { row($0, protectedFrom: protectedFrom) }
                                }
                            }
                            ForEach(plan.shown.dropFirst(fold?.count ?? 0), id: \.number) { row($0, protectedFrom: protectedFrom) }
                        }
                        // A range switch crossfades in place.
                        .id(plan.index)
                        .transition(.opacity)
                        .animation(.standard, value: plan.index)
                        .onChange(of: plan.index) { room.settle(plan.shown, again: true) }
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { lastRows = (entry.id, $0) }
                        // Until the list's width is known the rows would lay out at a guess, wide or
                        // narrow, and change a frame later.
                        .opacity(listWidth == nil ? 0 : 1)
                    } else {
                        EpisodePlaceholders(count: entry.episodes, metrics: metrics)
                            .frame(minHeight: lastRows.flatMap { $0.id == entry.id ? nil : $0.height }, alignment: .top)
                            .transition(.opacity)
                    }
                }
                .id(entry.id)
                .transition(.opacity)
            }
            .animation(.standard, value: loaded)
            .animation(.standard, value: entry.id)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { listWidth = $0 }
        // The room for text is settled when the rows are first laid out, and again each time the
        // page comes back: text that turned up meanwhile waits for that.
        .onChange(of: loadedTable, initial: true) { if loadedTable { room.settle(plan.shown) } }
        .onAppear { if loadedTable { room.settle(plan.shown, again: true) } }
        // A range switch, from the pop-up or the View menu, brings the pickers back into view
        // when they were scrolled away above.
        .onChange(of: plan.index) {
            guard pickers.top < Self.pickersHidden, let scroller else { return }
            withAnimation(.standard) { scroller.scrollTo(Self.pickers, anchor: UnitPoint(x: 0, y: 0.1)) }
        }
        // The View menu's Previous Episodes and Next Episodes act on this list while it is on
        // screen. A page covered by another keeps its visit, not the menu.
        .task(id: plan.ranges.isEmpty ? nil : EpisodeRangesShown(title: entry.id, index: plan.index, count: plan.ranges.count)) {
            navigator.episodeRangesShown = plan.ranges.isEmpty
                ? nil : EpisodeRangesShown(title: entry.id, index: plan.index, count: plan.ranges.count)
        }
        .onDisappear {
            if navigator.episodeRangesShown?.title == entry.id { navigator.episodeRangesShown = nil }
        }
        // The motion audit's range switch: the list scrolled to its last row first.
        .onChange(of: navigator.episodeListToEnd) {
            guard let last = plan.shown.last?.number else { return }
            scroller?.scrollTo(last, anchor: .bottom)
        }
        // Go to episode from the menu bar (⌘L), and the list asking for it while it is on screen.
        .task { navigator.goToShown = entry.id }
        .onDisappear { if navigator.goToShown == entry.id { navigator.goToShown = nil } }
        .onChange(of: navigator.goToAsked) { goToOpen = true }
        // A row landed: a near one scrolls to the top; another range, a long jump and Reduce
        // Motion crossfade to the list with the row already there.
        .onChange(of: navigator.goToRow[entry.id]) { _, landed in
            guard let landed, let scroller else { return }
            let switching = landed.range != nil && landed.range != plan.index
            let target = switching ? EpisodePlan(rows: rows, picked: landed.range) : plan
            let at = target.shown.firstIndex { $0.number == landed.number } ?? 0
            let average = lastRows.flatMap { $0.id == entry.id && !plan.shown.isEmpty ? $0.height / CGFloat(plan.shown.count) : nil } ?? 80
            let move = GoToMove.to(index: at, switching: switching, reduced: navigator.motionReduced || reduceMotion, pickersTop: pickers.top, rowHeight: average)
            // The watched run the row is folded into opens first.
            let opens = target.fold.map { at < $0.count } == true && !navigator.watchedShown(entry.id)
            switch move {
            case .scroll:
                if opens { navigator.toggleWatched(entry.id) }
                Task {
                    if opens { try? await Task.sleep(for: .milliseconds(300)) }
                    withAnimation(GoToMove.scrolling) { scroller.scrollTo(landed.number, anchor: UnitPoint(x: 0, y: 0.1)) }
                }
            case .crossfade:
                WindowCrossfade.run {
                    WindowCrossfade.still {
                        if switching, let range = landed.range { navigator.episodeRange[entry.id] = range }
                        if opens { navigator.toggleWatched(entry.id) }
                    }
                    // The rows of the range are laid out before the scroll finds the row.
                    try? await Task.sleep(for: .milliseconds(50))
                    WindowCrossfade.still { scroller.scrollTo(landed.number, anchor: UnitPoint(x: 0, y: 0.1)) }
                }
            }
        }
        // Digits typed into the focused list jump after a pause or Return.
        .focusable()
        .focusEffectDisabled()
        .onKeyPress(characters: .decimalDigits, phases: .down) { press in
            guard goToShown == entry.id else { return .ignored }
            type(press.characters, rows: rows, plan: plan)
            return .handled
        }
        .onKeyPress(.return) {
            guard navigator.typedEpisode?.title == entry.id else { return .ignored }
            jump(rows: rows, plan: plan)
            return .handled
        }
        // A screenshot's `-KaeriScroll <episode>`: that row scrolled near the top once drawn.
        .task(id: loadedTable) {
            guard loadedTable, let episode = navigator.launchScroll, let scroller else { return }
            navigator.launchScroll = nil
            try? await Task.sleep(for: .milliseconds(600))
            scroller.scrollTo(String(episode), anchor: UnitPoint(x: 0, y: 0.08))
        }
    }

    private var loadedTable: Bool { store.titles.episodes[entry.id] != nil }

    private var goToShown: AnilistId? { navigator.goToShown }

    /// Where Go to episode, from the popover or typed digits, came to a row.
    private func land(_ result: GoTo, rows: [EpisodeRow], plan: EpisodePlan) {
        guard case .row(let index, let range) = result else { return }
        goToOpen = false
        navigator.land(on: entry.id, rows: rows, index: Int(index), range: range.map(Int.init))
    }

    /// One more digit, and the jump after 0.6 s of quiet.
    private func type(_ digits: String, rows: [EpisodeRow], plan: EpisodePlan) {
        navigator.typeEpisode(digits, title: entry.id, rows: rows)
    }

    private func jump(rows: [EpisodeRow], plan: EpisodePlan) {
        navigator.jumpToTyped(title: entry.id, rows: rows)
    }

    private func row(_ row: EpisodeRow, protectedFrom: (row: String, after: Int)?) -> some View {
        VStack(spacing: 0) {
            if row.number == protectedFrom?.row {
                let shown = navigator.synopsisAllShown.contains(entry.id)
                ProtectionLine(episode: protectedFrom?.after ?? 0, shown: shown, phone: false) {
                    withAnimation(.standard) { navigator.showAllSynopses(entry.id, !shown) }
                }
            }
            EpisodeRowView(
                entry: entry, row: row, spoilers: store.spoilerProtection(for: entry.id),
                metrics: metrics, room: room.room
            )
        }
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
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let row: EpisodeRow
    let spoilers: Bool
    let metrics: EpisodeMetrics
    /// Whether the table keeps room for text.
    let room: Bool

    private var next: UInt32 { entry.progress?.next ?? 1 }

    /// Whether protection hides this row's still and synopsis now.
    private var hidden: Bool {
        EpisodeProtection.hidden(
            row, spoilers: spoilers,
            revealed: navigator.synopsisShown[entry.id] ?? [], all: navigator.synopsisAllShown.contains(entry.id)
        )
    }

    var body: some View {
        if row.isUnaired(), let date = row.airsDate {
            UnairedEpisodeRow(entry: entry, row: row, airs: date, metrics: metrics, textInset: textInset)
        } else {
            VStack(spacing: 0) {
                airedRow
                    #if os(macOS)
                    // A synopsis cut at three lines is whole here, as a cut title is.
                    .tooltip(helpText)
                    #endif
                DiscussionLine(entry: entry, row: row, inset: textInset)
            }
        }
    }

    /// Where the row's words start, from its leading edge.
    private var textInset: CGFloat {
        8 + 22 + 14 + metrics.still.width + (metrics.text == .beside ? 14 : 12)
    }

    #if os(macOS)
    private var helpText: String {
        if room, !hidden, let text = row.description, !text.isEmpty { return text }
        return row.airsDate.map(AirsWords.aired) ?? ""
    }
    #endif

    @ViewBuilder
    private var airedRow: some View {
        let number = UInt32(row.number) ?? 0
        let current = number == next
        // One no source has: nothing to play, so a click offers what can be done, adding a
        // source among it, rather than playing.
        if let link = store.licensed.episodeLink(entry.id, number) {
            // On a service they use: the row opens the service, and says so with the glyph.
            Button {
                store.openService(entry.id, link: ServiceLink(service: link.service, url: link.url, notes: nil), episode: number)
            } label: {
                rowLabel(current: current, external: true)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: spoken(current: current), hint: [L.licensedOpensOutside, hint].compactMap { $0 }.joined(separator: ". ")) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
        } else if row.metadataOnly || row.unsourced {
            Menu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } label: {
                rowLabel(current: current)
            }
            .menuStyle(.button)
            .menuIndicator(.hidden)
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: spoken(current: current), hint: hint) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
        } else {
            Button {
                store.send(.play(id: entry.id, episode: row.number))
            } label: {
                rowLabel(current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: spoken(current: current), hint: hint) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            #if os(iOS)
            .contextMenu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } preview: {
                EpisodePreview(entry: entry, row: row, hidden: hidden)
                    .environment(store)
                    .environment(navigator)
            }
            #else
            .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
            #endif
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

    private func rowLabel(current: Bool, external: Bool = false) -> some View {
        EpisodeRowLabel(
            entry: entry, row: row, metrics: metrics, room: room, hidden: hidden, current: current,
            numberColumn: true, asked: navigator.synopsisIsShown(entry.id, row.number), external: external
        )
        .opacity(row.unsourced && store.licensed.episodeLink(entry.id, UInt32(row.number) ?? 0) == nil ? 0.55 : 1)
        .background(current ? Palette.selection : landed ? Palette.raisedSelected : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
    }

    /// The row Go to episode landed on keeps the selection fill until another lands or the page goes.
    private var landed: Bool { navigator.goToRow[entry.id]?.number == row.number }
}

/// The next episode still to air: no button, since nothing can play it; the show's banner crop in
/// its still's place, its title in haze, and when it airs in water ("Airs Thursday 16:30").
struct UnairedEpisodeRow: View {
    let entry: Entry
    let row: EpisodeRow
    let airs: Date
    var metrics = EpisodeMetrics.macWide
    var textInset: CGFloat = 0

    var body: some View {
        let title = row.title ?? L.episodeNumberString(number: row.number)
        let when = AirsWords.airs(airs)
        HStack(spacing: 14) {
            Text(row.number)
                .font(EpisodeRowLabel.numberFont(metrics))
                .monospacedDigit()
                .foregroundStyle(.haze)
                .lineLimit(1)
                .fixedSize()
                .frame(width: 22, alignment: .trailing)
            // The show's banner crop: the episode has no still before it airs.
            EpisodeStill(entry: entry, row: row, hidden: false, unaired: true, size: metrics.still)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(EpisodeRowLabel.titleFont(metrics)).foregroundStyle(.haze).lineLimit(2)
                HStack(spacing: 6) {
                    Circle().fill(.water).frame(width: 6, height: 6)
                    Text(when).monospacedDigit()
                }
                .font(EpisodeRowLabel.factsFont(metrics))
                .foregroundStyle(.water)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Color.clear.frame(width: 14)
        }
        .padding(.horizontal, 8)
        .frame(minHeight: metrics.short)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.titleEpisodeRowUnaired(title: row.title.map { L.episodeTitledText(number: row.number, title: $0) } ?? L.episodeNumberString(number: row.number), when: when))
    }
}

/// The episode rows while the table loads: as many as the title has, where that is known, up to
/// a screenful, so the rows that replace them take the room they kept.
struct EpisodePlaceholders: View {
    let count: UInt32?
    var metrics = EpisodeMetrics.macWide

    var body: some View {
        VStack(spacing: 0) {
            ForEach(0..<Int(min(max(count ?? 4, 1), 12)), id: \.self) { _ in EpisodePlaceholderRow(metrics: metrics) }
        }
    }
}

/// An episode row while the table loads.
struct EpisodePlaceholderRow: View {
    var metrics = EpisodeMetrics.macWide

    var body: some View {
        HStack(spacing: 14) {
            Text("7").frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: metrics.still.width, height: metrics.still.height)
            // A redacted title and facts line, and no synopsis lines: a show without synopses
            // never promises them.
            VStack(alignment: .leading, spacing: 2) {
                Text("The Lantern Road") // i18n-skip
                Text("24 min · Aired 21 May 2014") // i18n-skip
                    .font(EpisodeRowLabel.factsFont(metrics))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(EpisodeRowLabel.titleFont(metrics))
        .foregroundStyle(.haze)
        .redacted(reason: .placeholder)
        .padding(.horizontal, 8)
        .frame(height: metrics.short)
        .accessibilityHidden(true)
    }
}

/// The synopsis, the facts and the sheet's opener beside the episodes: five lines on the
/// Mac, three lines on iPad with "More" to show the rest in place.
struct TitleDetails: View {
    let entry: Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let synopsis = entry.shownSynopsis
            if !synopsis.isEmpty {
                #if os(macOS)
                InlineMoreText(text: synopsis, lines: 5, font: .mac(13), lineHeight: 20)
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
        // Another season's synopsis and facts swap in place, by one short fade.
        .contentTransition(.opacity)
        .animation(.quick, value: entry.id)
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
            AnotherButton(id: id)
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
