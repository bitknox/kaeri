//! Routing: each command to whatever carries it out, what the engine side does on the way of an
//! event, and the work it leaves itself.
//!
//! The engine's actor drives these, behind `Engine::send` and the engine's event stream:
//! [`dispatch`] for each command, [`receive`] for each event on its way to the front end, and
//! [`handle_internal`] for each message the engine side left itself. No front end calls them.

use kaeri_meta::Meta;
use kaeri_model::StoreMsg;
use kaeri_model::{
    Msg, Span,
    command::{Command, LicensedCommand},
    event::Event,
    notice::Notice,
    settings::SettingKey,
};
use kaeri_providers::ProviderRegistry;
use kaeri_store::Store;

use crate::playback::start::{control_playback, control_player, play, play_local, play_source};
use crate::syncing::Ctx;
use crate::{
    artwork,
    catalog::{
        browse,
        calendar::{load_calendar, load_calendar_ahead, load_calendar_past},
        detail::{load_detail, open_external, play_trailer},
        home::{self, HomeAsk, HomeMemory},
        lists::{hide_from_continue, load_continue, load_seasonal},
        probe, recents, search,
        up_next::{self, Ask},
    },
    data,
    downloads::{
        self, ahead,
        commands::{
            download_cancel, download_clear_completed, download_delete, download_episode,
            download_many, download_pause, download_start_now,
        },
    },
    jobs::JobGuard,
    outbox::{Internal, Outbox},
    playback::{prefetch::prefetch, resolve::load_sources},
    plugin_stores::tasks::{StoreTaskContext, store_task},
    proxy::{check_torrent_proxy, set_torrent_proxy},
    services::Services,
    settings::{set_setting, set_translation, step_setting},
    show_settings::{
        load_show_settings, reset_show_settings, set_show_setting, set_show_subtitles,
    },
    sources::{
        self,
        commands::{check_providers, disable_plugin},
        torrent::reload_sources,
    },
    state::EngineState,
    titles::{
        episodes::{list_episodes, set_watched},
        matching::{clear_match, fix_match, manual_search, set_provider_preference},
    },
    tracking::{
        self,
        commands::{
            cancel_sign_in, connect, connect_kitsu, finish_sign_in, load_library,
            load_title_language, set_score, set_status, sync_now,
        },
    },
};

/// What a task run by [`spawn`] is lent: its own clones of the handles it reads, and of the
/// outbox it answers on.
#[derive(Clone)]
pub struct Lent {
    pub meta: Meta,
    pub store: Store,
    pub registry: ProviderRegistry,
    /// The plugin stores joined, which say where each plugin came from.
    pub stores: crate::plugin_stores::Stores,
    pub tx: Outbox,
}

/// The handles a task is lent, cloned.
pub(crate) fn lend(services: &Services, tx: &Outbox) -> Lent {
    Lent {
        meta: services.meta.clone(),
        store: services.store.clone(),
        registry: services.registry.clone(),
        stores: services.stores.clone(),
        tx: tx.clone(),
    }
}

/// The licensed services commands. Settings are kept in the database with no job; the title's
/// availability reads AniList through one.
fn licensed_command(command: LicensedCommand, services: &Services, tx: &Outbox) {
    match command {
        LicensedCommand::Load { system_region } => {
            crate::licensed::remember_system_region(system_region.as_deref(), &services.store);
            publish_licensed(services, tx);
        }
        LicensedCommand::SetServices { services: chosen } => {
            crate::licensed::set_services(chosen, &services.store);
            publish_licensed(services, tx);
        }
        LicensedCommand::SetRegion { region } => {
            crate::licensed::set_region(region, &services.store);
            publish_licensed(services, tx);
        }
        LicensedCommand::SetShowOthers { show } => {
            crate::licensed::set_show_others(show, &services.store);
            publish_licensed(services, tx);
        }
        LicensedCommand::LoadAvailability { id, plays_here } => {
            spawn(None, services, tx, move |lent| {
                crate::licensed::load_availability(lent, id, plays_here)
            })
        }
        LicensedCommand::Open { id, service, url, episode } => {
            crate::licensed::open(id, service, url, episode, services, tx)
        }
        LicensedCommand::Returned => crate::licensed::returned(services, tx),
    }
}

/// Send the Licensed services page, once AniList's list of services is in hand.
fn publish_licensed(services: &Services, tx: &Outbox) {
    let now = services.clock.now();
    spawn(None, services, tx, move |lent| crate::licensed::publish(lent, now));
}

/// List title `id`'s episodes again, quietly: no activity row, as a source change is not
/// something the viewer asked a title for.
pub(crate) fn list_episodes_again(
    id: kaeri_core::ids::AnilistId,
    announce: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let ticket = engine.listed.ticket(id);
    let table = engine.dubs.table(id);
    let naming = engine.names.naming(id, engine.network.offline, &services.clock);
    spawn(None, services, tx, move |lent| async move {
        list_episodes(lent, id, naming, ticket, announce).await;
        drop(table);
    });
}

/// Run a task off the UI thread, once the AniList rate limit lets it, and let it report back
/// through the channel.
fn spawn<W, F>(job: Option<JobGuard>, services: &Services, tx: &Outbox, work: W)
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    spawn_abortable(job, services, tx, work);
}

/// [`spawn`], with the handle that stops it: a search a newer one replaces. Stopped while it
/// waits on the limit, it spends nothing.
pub(crate) fn spawn_abortable<W, F>(
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
    work: W,
) -> tokio::task::AbortHandle
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let Lent { meta, store, registry, stores, tx } = lend(services, tx);

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang. As a note on the job it
        // counts down and goes away with the work, where a status line kept saying "12s" after.
        if let Some(wait) = meta.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            match &job {
                Some(job) => job.note(Msg::Store(StoreMsg::JobAnilistLimit), Some(wait)),
                // No job to note it on. A toast, which goes when it is done: as a status it
                // stayed, still counting "12s", after the wait was over.
                None => {
                    let _ = tx.send(Event::Toast(Notice::info(Msg::Store(
                        StoreMsg::NoticeAnilistLimit {
                            wait: Span::countdown(wait.as_secs().max(1) as i64),
                        },
                    ))));
                }
            }
        }
        work(Lent { meta, store, registry, stores, tx }).await;
    })
    .abort_handle()
}

/// Route a command to whatever carries it out.
///
/// One arm for every command, and each names what runs it, so a command added to the
/// protocol does not build until it is routed. What asks AniList or the sources and answers
/// once runs through [`spawn`], behind the AniList limit. Playback runs on its own: it owns
/// the live player's control channel, and replacing that sender is what stops a finished
/// episode's keys steering the next one.
///
/// The actor runs it for each command `Engine::send` hands over, one at a time.
/// What "Up next for you" is told of the viewer and the moment.
fn up_next_ask(engine: &EngineState, services: &Services) -> Ask {
    Ask {
        listed: downloads::ahead::listed(services),
        adult: engine.config.effective().meta.adult,
        now: services.clock.now(),
        zone: services.clock.zone(),
    }
}

/// What a load of Home is told of the viewer and the moment.
fn home_ask(engine: &EngineState, services: &Services, refresh: bool) -> HomeAsk {
    HomeAsk {
        listed: downloads::ahead::listed(services),
        adult: engine.config.effective().meta.adult,
        now: services.clock.now(),
        zone: services.clock.zone(),
        offline: engine.network.offline,
        refresh,
        sources: !services.registry.usable_ids().is_empty(),
        usable: probe::Usable::of(&services.registry),
        demo: crate::demo::state(engine, services),
    }
}

/// The sources changed, so a miss kept from the ones before may no longer hold. A title the
/// apps were told is not found this session is told what it is now, and asked again when
/// interest is shown in it, and a Home open goes out again with its marks as they now stand.
pub(crate) fn home_after_sources(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    let usable = probe::Usable::of(&services.registry);
    let now = services.clock.now();
    let probed: Vec<_> = engine.probed.keys().copied().collect();
    for id in probed {
        if let Some(playable) = probe::lapsed(&services.store, id, &usable, now) {
            engine.probed.remove(&id);
            let _ = tx.send(Event::TitlePlayable { id, playable });
        }
    }
    if engine.home.asked() {
        let (ask, memory) = (home_ask(engine, services, false), engine.home.clone());
        spawn(None, services, tx, move |lent| async move {
            home::changed(&lent.meta, &lent.store, &lent.tx, &ask, &memory);
        });
    }
}

/// After a dismissal or its Undo: Home again from what is kept, and the hidden titles when
/// Settings has them open.
async fn after_dismissal(
    meta: Meta,
    store: Store,
    tx: Outbox,
    ask: HomeAsk,
    memory: HomeMemory,
) {
    home::changed(&meta, &store, &tx, &ask, &memory);
    if memory.hidden_asked() {
        home::hidden_titles(&meta, &store, &tx).await;
    }
}

/// A title's episode table; `low` when nobody is waiting on it, as for a card's stills.
fn episodes_job(
    id: kaeri_core::ids::AnilistId,
    low: bool,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    engine.listed.note(id);
    // A lookup of the title's dub waits for this.
    let table = engine.dubs.table(id);
    // Named from another source where its own left titles and stills out: see `titles::names`.
    let naming = engine.names.naming(id, engine.network.offline, &services.clock);
    let ticket = engine.listed.ticket(id);
    spawn(job, services, tx, move |lent| async move {
        let listing = list_episodes(lent, id, naming, ticket, true);
        if low {
            kaeri_meta::in_background(listing).await;
        } else {
            listing.await;
        }
        drop(table);
    })
}

pub(crate) fn dispatch(
    task: Command,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match task {
        // The lists, the calendar, search and a title's page.
        Command::LoadContinue => spawn(job, services, tx, load_continue),
        Command::LoadSeasonal => {
            let adult = engine.config.effective().meta.adult;
            spawn(job, services, tx, move |lent| load_seasonal(lent, adult))
        }
        Command::HideFromContinue { id } => hide_from_continue(id, services, tx),
        Command::LoadUpNext => {
            engine.up_next_queue = Some(up_next::queue_mark(&services.store));
            let ask = up_next_ask(engine, services);
            spawn(job, services, tx, move |lent| up_next::load_up_next(lent, ask))
        }
        // Not interested, and its Undo, on Up next for you, Home and Settings, Hidden titles.
        Command::RestoreUpNext { id } => {
            let ask = up_next_ask(engine, services);
            let (home, memory) = (home_ask(engine, services, false), engine.home.clone());
            spawn(job, services, tx, move |lent| async move {
                let (meta, store, tx) =
                    (lent.meta.clone(), lent.store.clone(), lent.tx.clone());
                up_next::restore(lent, id, ask).await;
                after_dismissal(meta, store, tx, home, memory).await;
            })
        }
        Command::DismissUpNext { id } => {
            let (home, memory) = (home_ask(engine, services, false), engine.home.clone());
            spawn(job, services, tx, move |lent| async move {
                let (meta, store, tx) =
                    (lent.meta.clone(), lent.store.clone(), lent.tx.clone());
                up_next::dismiss(lent, id).await;
                after_dismissal(meta, store, tx, home, memory).await;
            })
        }
        Command::LoadHome { refresh } => {
            engine.home_mark = Some(home::queue_mark(&services.store));
            let (ask, memory) = (home_ask(engine, services, refresh), engine.home.clone());
            spawn(job, services, tx, move |lent| home::load(lent, memory, ask))
        }
        Command::LoadHiddenTitles => {
            engine.home.ask_hidden();
            spawn(job, services, tx, |lent| async move {
                home::hidden_titles(&lent.meta, &lent.store, &lent.tx).await;
            })
        }
        Command::ProbeTitle { id } => {
            let now = services.clock.now();
            // Asked once a minute at most, the answer kept a day: interest shown twice in a row
            // is one check.
            if engine.probed.get(&id).is_some_and(|at| now - at < 60 && now >= *at) {
                drop(job);
                return;
            }
            engine.probed.insert(id, now);
            let translation = engine.config.effective().playback.translation;
            let (ask, memory) = (home_ask(engine, services, false), engine.home.clone());
            spawn(job, services, tx, move |lent| async move {
                let Lent { meta, store, registry, tx, .. } = lent;
                probe::probe(&meta, &store, &registry, &tx, id, translation, now).await;
                if memory.shows(id) {
                    home::changed(&meta, &store, &tx, &ask, &memory);
                }
            })
        }
        // Each row marked with its dub where a title page found it: see `Entry::airing_dubbed`.
        Command::LoadCalendar | Command::RefreshCalendar => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            let clock = services.clock.clone();
            let remembered = matches!(task, Command::LoadCalendar);
            spawn(job, services, tx, move |lent| {
                load_calendar(lent, dubs, memory, clock, remembered)
            })
        }
        Command::LoadCalendarPast { anchor, page, through } => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            spawn(job, services, tx, move |lent| {
                load_calendar_past(lent, dubs, memory, anchor, page, through)
            })
        }
        Command::LoadCalendarAhead { anchor, page, through } => {
            let (dubs, memory) = (engine.dubs.clone(), engine.calendar.clone());
            spawn(job, services, tx, move |lent| {
                load_calendar_ahead(lent, dubs, memory, anchor, page, through)
            })
        }
        Command::Search { query } => search::submit(query, job, engine, services, tx),
        Command::SearchLive { query } => search::live(query, engine, services, tx),
        Command::SearchMore { query, page } => {
            let (searches, adult) =
                (engine.search.clone(), engine.config.effective().meta.adult);
            spawn(job, services, tx, move |lent| {
                search::more(lent, searches, query, page, adult)
            })
        }
        Command::LoadBrowse => {
            let (adult, now) = (engine.config.effective().meta.adult, services.clock.now());
            // The period chosen for the Trending rail, shown from what is kept: never asked
            // for at launch, and asked again behind the screen once old.
            let offline = engine.network.offline;
            spawn(job, services, tx, move |lent| async move {
                let more = lent.clone();
                tokio::join!(
                    browse::load_browse(lent, adult, now),
                    crate::catalog::trending::load_remembered(more, adult, now, offline),
                );
            })
        }
        Command::Surprise(pool) => {
            let adult = engine.config.effective().meta.adult;
            let (offline, now) = (engine.network.offline, services.clock.now());
            let picks = engine.picks.clone();
            spawn(job, services, tx, move |lent| async move {
                crate::catalog::surprise::pick(
                    lent,
                    picks,
                    pool,
                    adult,
                    offline,
                    now,
                    &crate::catalog::surprise::random,
                )
                .await;
            })
        }
        Command::LoadTrending(period) => {
            let (adult, now) = (engine.config.effective().meta.adult, services.clock.now());
            let offline = engine.network.offline;
            spawn(job, services, tx, move |lent| {
                crate::catalog::trending::load(lent, period, adult, now, offline)
            })
        }
        Command::LoadSearchFacets => {
            let (adult, now) = (engine.config.effective().meta.adult, services.clock.now());
            spawn(job, services, tx, move |lent| browse::load_facets(lent, adult, now))
        }
        Command::LoadRecentSearches => recents::publish(services, tx),
        Command::RememberSearch(query) => recents::remember(query, services, tx),
        Command::ForgetRecentSearch(query) => recents::forget(query, services, tx),
        Command::ClearRecentSearches => recents::clear(services, tx),
        Command::LoadDetail(id) => {
            // Another title's page: the episode names looked up for the one left stop.
            engine.names.leave_for(id);
            // "Also a book" shows only to someone who both watches and reads.
            let both = engine.reading.answer() == kaeri_model::medium::KaeriIsFor::Both;
            spawn(job, services, tx, move |lent| load_detail(lent, id, both))
        }
        Command::LoadCards { ids } => {
            spawn(job, services, tx, move |lent| crate::catalog::cards::load_cards(lent, ids))
        }
        Command::PrepareCards { ids } => spawn(job, services, tx, move |lent| {
            crate::catalog::cards::prepare_cards(lent, ids)
        }),
        Command::LoadTitleLanguage => load_title_language(services, tx),
        Command::PlayTrailer { url, title } => {
            play_trailer(url, title, job, engine, services, tx)
        }
        Command::OpenExternal { url } => open_external(url, services, tx),

        // Reading's catalogue and Fix this match for a book folder: see `read_catalog`. The
        // rest of reading is further down.
        Command::Reading(command) if crate::read_catalog::handles(&command) => {
            crate::read_catalog::command(engine, services, command, tx)
        }
        Command::BookSources(kaeri_model::book_sources::BookSourceCommand::FindMatch {
            group,
            query,
        }) if engine.reading.available => {
            crate::read_catalog::find_match(engine, services, group, query, tx)
        }

        // The licensed services: kept in the database, with no job.
        Command::Licensed(command) => licensed_command(command, services, tx),
        Command::Local(command) => {
            let order = engine.config.effective().providers.order.clone();
            crate::local::command(command, services, order, tx)
        }
        Command::Server(command) => {
            let order = engine.config.effective().providers.order.clone();
            crate::servers::command(command, services, order, tx)
        }

        // Reading: refused in place while it ships dark. See `reading`.
        Command::Reading(kaeri_model::reading::ReadingCommand::LoadTracking { work })
            if engine.reading.available =>
        {
            crate::read_tracking::load(services, work.clone(), tx);
            crate::read_tracking::resolve_for(services, &work, tx);
        }
        Command::Reading(kaeri_model::reading::ReadingCommand::ChooseMapping {
            work,
            tracker,
            tracker_id,
        }) if engine.reading.available => {
            crate::read_tracking::choose(services, work, &tracker, tracker_id, tx)
        }
        Command::Reading(kaeri_model::reading::ReadingCommand::ResolveConflict {
            work,
            tracker,
            keep_local,
        }) if engine.reading.available => {
            crate::read_tracking::resolve(services, work, &tracker, keep_local, tx)
        }
        Command::Reading(kaeri_model::reading::ReadingCommand::RemoveFromLists { work })
            if engine.reading.available =>
        {
            crate::read_tracking::remove_from_lists(services, work.clone(), tx);
            let ask = crate::read_catalog::Ask::of(engine, services);
            let kit = crate::read_catalog::Kit::of(services);
            crate::read_catalog::series::header(&kit, ask, &work, tx);
            crate::read_tracking::load(services, work, tx);
        }
        Command::Reading(kaeri_model::reading::ReadingCommand::ConnectMangaUpdates {
            username,
            password,
        }) if engine.reading.available => {
            crate::read_tracking::connect_mangaupdates(services, username, password.0, tx)
        }
        Command::Reading(command) => crate::reading::reading(engine, services, command, tx),
        Command::Reader(command) => crate::reading::reader(engine, services, command, tx),
        // Read from and "Not found in" search with the catalogue's names for the work.
        Command::BookSources(
            command @ (kaeri_model::book_sources::BookSourceCommand::LoadForWork { .. }
            | kaeri_model::book_sources::BookSourceCommand::SearchSourceForWork {
                ..
            }
            | kaeri_model::book_sources::BookSourceCommand::ChooseSourceSeries {
                ..
            }),
        ) if engine.reading.available => crate::read_catalog::editions::command(
            &crate::read_catalog::Kit::of(services),
            crate::read_catalog::Ask::of(engine, services),
            command,
            tx,
        ),
        Command::BookSources(command) => {
            crate::reading::book_sources(&engine.reading, services, command, tx)
        }
        Command::Medium(command) => crate::reading::medium(engine, services, command, tx),

        // An episode's discussion: off the loop, with no job, each answered with the
        // discussion whole. See `discussion`.
        Command::OpenDiscussion { media, episode } => {
            crate::discussion::open(media, episode, engine, services, tx)
        }
        Command::MoreComments => crate::discussion::more(engine, services, tx),
        Command::PostComment { parent, text } => {
            crate::discussion::post(parent, text, engine, services, tx)
        }
        Command::EditComment { id, text } => {
            crate::discussion::edit(id, text, engine, services, tx)
        }
        Command::DeleteComment { id } => crate::discussion::delete(id, engine, services, tx),
        Command::LikeComment { id } => crate::discussion::like(id, engine, services, tx),
        Command::CloseDiscussion => crate::discussion::close(engine),

        // A title's episodes, and the source match they come from.
        // Quietly, with no job, and never in the way of the table or a play: see
        // `titles::dub`.
        Command::LoadDub(id) => engine.dubs.ask(id, engine.network.offline, services, tx),
        Command::LoadEpisodes(id) => episodes_job(id, false, job, engine, services, tx),
        // The same table for a card's stills, asked of AniList at low priority.
        Command::LoadStills(id) => episodes_job(id, true, job, engine, services, tx),
        Command::SetWatched { id, episodes, watched } => {
            set_watched(id, &episodes, watched, engine, services, tx);
            // One episode marked watched by hand is one finished: that show, topped up. A mark
            // of many, such as a whole season, is not.
            if watched && episodes.len() == 1 {
                ahead::top_up(id, engine, services, tx);
            }
        }
        // A title matched afresh may be another title on the source, so its dub is too.
        Command::ClearMatch { id } => {
            engine.dubs.forget(id, &services.store);
            engine.names.forget(id);
            spawn(job, services, tx, move |lent| clear_match(lent, id))
        }
        Command::FixMatch { id, provider_id, key, title } => {
            engine.dubs.forget(id, &services.store);
            engine.names.forget(id);
            spawn(job, services, tx, move |lent| fix_match(lent, id, provider_id, key, title))
        }
        Command::ManualSearch { id, query, provider_id } => {
            manual_search(id, query, provider_id, job, engine, services, tx)
        }
        Command::SetProviderPreference { id, provider } => {
            engine.dubs.forget(id, &services.store);
            engine.names.forget(id);
            set_provider_preference(id, provider, engine, services, tx);
            // The pin applies to what is on screen now, not the next time it opens.
            crate::titles::refresh::relist(id, true, engine, services, tx);
        }

        // Playback.
        Command::Prefetch { id, episode } => prefetch(id, episode, engine, services),
        // Each refused where this build has no player to play in: see `playback::refuse`.
        // A play wants the source's time: a dub or episode names lookup running stops for it.
        Command::Play { id, episode, from_start } => {
            #[cfg(feature = "party")]
            if crate::party::play_in_room(id, &episode, engine, services) {
                return;
            }
            engine.dubs.stop();
            engine.names.stop();
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play(id, episode, from_start, job, engine, services, tx)
        }
        // The demo: added or taken out of the chain, or played from part 1 by the ordinary play.
        Command::AddDemo => crate::demo::set(true, engine, services, tx),
        Command::RemoveDemo => crate::demo::set(false, engine, services, tx),
        Command::PlayDemo => {
            if !engine.config.effective().sources.demo {
                crate::demo::set(true, engine, services, tx);
            }
            engine.dubs.stop();
            engine.names.stop();
            let id = kaeri_core::demo::DEMO_ANILIST_ID;
            play(id, "1".into(), false, job, engine, services, tx)
        }
        Command::ResetSubtitleStyle => crate::subtitles::reset(engine, services, tx),
        Command::PlaySource { id, episode, provider_id, source_id } => {
            engine.dubs.stop();
            engine.names.stop();
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play_source(id, episode, provider_id, source_id, job, engine, services, tx)
        }
        // Watch parties, where this build has them: see `party`.
        #[cfg(feature = "party")]
        Command::PlayParty { id, episode, server, room, name } => {
            engine.dubs.stop();
            engine.names.stop();
            crate::party::play(id, episode, (server, room, name), job, engine, services, tx)
        }
        #[cfg(feature = "party")]
        Command::LoadPartyForm => crate::party::form(engine, tx),
        #[cfg(feature = "party")]
        Command::PartyJoin { server, room, name } => {
            crate::party::join(&server, &room, name.as_deref(), engine, services, tx);
        }
        #[cfg(feature = "party")]
        Command::PartyReady { ready } => crate::party::ready(ready, engine, tx),
        #[cfg(feature = "party")]
        Command::PartyLeave => crate::party::leave(engine, tx),
        #[cfg(feature = "party")]
        Command::Party(command) => crate::party::command(command, engine, services, tx),
        #[cfg(not(feature = "party"))]
        Command::PlayParty { .. }
        | Command::LoadPartyForm
        | Command::PartyJoin { .. }
        | Command::PartyReady { .. }
        | Command::PartyLeave
        | Command::Party(_) => {
            let _ =
                tx.send(Event::Toast(Notice::alert(Msg::Store(StoreMsg::NoticePartyNotBuilt))));
        }
        #[cfg(feature = "cast")]
        Command::CastTo { device, id, episode, position } => {
            engine.dubs.stop();
            engine.names.stop();
            crate::playback::start::cast_to(
                &device, id, episode, position, job, engine, services, tx,
            );
        }
        #[cfg(feature = "cast")]
        Command::StopCasting => crate::playback::start::stop_casting(engine, services),
        #[cfg(not(feature = "cast"))]
        Command::CastTo { .. } | Command::StopCasting => {
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeCastUnavailable)));
        }
        // A link the viewer opened: see `opened`.
        Command::Opened(command) => crate::opened::run(command, job, engine, services, tx),
        Command::PlayLocal { id, episode, title, path } => {
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            play_local(id, episode, title, path, job, engine, services, tx)
        }
        Command::LoadSourceOptions(id) => {
            crate::sources::options::load_source_options(id, job, engine, services, tx)
        }
        Command::PinRelease { id, provider_id, release_id, title } => {
            crate::sources::options::pin_release(
                id,
                kaeri_store::ReleasePin { provider_id, release_id, title },
                engine,
                services,
                tx,
            )
        }
        Command::LoadSources { id, episode } => {
            load_sources(id, episode, job, engine, services, tx)
        }
        // Nothing to steer where nothing plays: no playback has left its controls.
        Command::Player(command) => control_player(command, engine),
        // The same, aimed: let go when another playback has followed the one it names.
        Command::Control { playback, command } => control_playback(playback, command, engine),
        // Where the app is and its network: which torrents wait, and whether keep next runs
        // for the shows it was asked for while downloads waited.
        Command::AppPhase { phase } => {
            services.sync.pulls.set_away(phase != kaeri_model::command::AppState::Foreground);
            crate::phases::app_phase(phase, engine);
            if phase != kaeri_model::command::AppState::Foreground {
                let EngineState { config, reader, .. } = engine;
                let cx = reader.cx(config, services, tx);
                reader.background(&cx);
            }
            ahead::resume(engine, services, tx);
            crate::downloads::books::resume(engine, services, tx);
        }
        Command::NetworkPath { cellular, constrained, offline } => {
            let network = crate::phases::Network { cellular, constrained, offline };
            let moved = engine.network.offline != offline;
            services.offline.store(offline, std::sync::atomic::Ordering::Relaxed);
            services.connection.send_replace(offline);
            crate::phases::network_path(network, engine, tx);
            ahead::resume(engine, services, tx);
            crate::downloads::books::resume(engine, services, tx);
            // Home open says it is offline, and since when, or stops saying so, at once.
            if moved && engine.home.asked() {
                let (ask, memory) = (home_ask(engine, services, false), engine.home.clone());
                spawn(job, services, tx, move |lent| async move {
                    home::changed(&lent.meta, &lent.store, &lent.tx, &ask, &memory);
                });
            }
        }
        Command::DeviceState {
            reduce_motion,
            autoplay_previews,
            low_power,
            thermal_serious,
            screen_reader,
            low_memory,
        } => {
            let device = kaeri_model::trailer::Device {
                reduce_motion,
                autoplay_previews,
                low_power,
                thermal_serious,
                screen_reader,
                low_memory,
            };
            crate::autoplay::device_state(device, engine, tx);
            // Finding skip times pauses in a power saving mode, which Settings says.
            services.skip_find.set_low_power(low_power);
            crate::skip_find::refresh(&services.skip_find, engine, tx);
        }
        // Taken in as it is sent by `Engine::send`; one routed here all the same lands.
        Command::PlayerReport { playback, report } => services.hosted.report(playback, report),

        // The sources.
        Command::CheckProviders => spawn(job, services, tx, check_providers),
        Command::CheckProxy => {
            engine.torrent_tasks.check_now(engine.config.effective(), services, tx);
        }

        // The trackers.
        Command::SyncNow => sync_now(job, services, tx),
        Command::SyncVisible { visible } => {
            services.sync.pulls.set_away(!visible);
            drop(job);
        }
        Command::Connect { tracker } => connect(tracker, job, services, tx),
        Command::FinishSignIn { tracker, callback } => {
            finish_sign_in(tracker, callback, job, services, tx)
        }
        Command::CancelSignIn { tracker } => cancel_sign_in(tracker, services, tx),
        Command::ConnectKitsu { username, password } => {
            connect_kitsu(username, password, job, services, tx)
        }
        Command::LoadProfiles => {
            let _ = tx
                .send(Event::Profiles(crate::profile::view(&services.store, &services.paths)));
            drop(job);
        }
        Command::AddProfile { name, kids, first_name } => {
            let first = first_name.unwrap_or_else(|| "Me".to_owned());
            if let Err(e) =
                crate::profile::add(&services.store, &services.paths, &name, kids, &first)
            {
                tracing::warn!(error = %e, "could not add a profile");
            }
            let _ = tx
                .send(Event::Profiles(crate::profile::view(&services.store, &services.paths)));
            drop(job);
        }
        Command::EditProfile { id, name, kids } => {
            if let Err(e) = crate::profile::edit(&services.paths, &id, &name, kids) {
                tracing::warn!(error = %e, "could not edit a profile");
            }
            let _ = tx
                .send(Event::Profiles(crate::profile::view(&services.store, &services.paths)));
            drop(job);
        }
        Command::SwitchProfile { id } => {
            match crate::profile::choose(&services.paths, &id) {
                Ok(()) => {
                    let _ = tx.send(Event::ProfileSwitch { id });
                }
                Err(e) => tracing::warn!(error = %e, "could not switch profile"),
            }
            drop(job);
        }
        Command::DeleteProfile { id } => {
            let promoted_to = services.store.active_profile();
            let primary_goes =
                kaeri_store::profile::ProfileList::load(&services.paths.data_dir)
                    .ok()
                    .flatten()
                    .is_some_and(|list| list.primary == id);
            match crate::profile::delete(
                &services.store,
                &services.paths,
                &services.tokens,
                &id,
            ) {
                // The one in use took the first profile's place: its tokens and settings are the
                // bare ones now, which a restart on it reads.
                Ok(()) if primary_goes => {
                    if let Some(id) = promoted_to {
                        let _ = tx.send(Event::ProfileSwitch { id });
                    }
                }
                Ok(()) => {}
                Err(e) => tracing::warn!(error = %e, "could not delete a profile"),
            }
            let _ = tx
                .send(Event::Profiles(crate::profile::view(&services.store, &services.paths)));
            drop(job);
        }
        Command::LoadSync => {
            let _ = tx.send(Event::SyncStatus(crate::syncing::status(&Ctx::of(services))));
            drop(job);
        }
        Command::StartSync { device_name, folder } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::start(&ctx, &tx, device_name, folder).await;
            });
        }
        Command::JoinSync { device_name, folder, recovery_key } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::join(&ctx, &tx, device_name, folder, &recovery_key.0).await;
            });
        }
        Command::SyncSignIn { service } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::sign_in_again(&ctx, &tx, service).await;
            });
        }
        Command::FinishSyncSignIn { service, callback } => {
            if let Some(cloud) = crate::cloud::Service::of(&service)
                && !services.syncing.cloud.deliver(cloud, callback.0)
            {
                // A page that came back after its sign-in ended: said, never dropped quietly.
                let expired = kaeri_model::group::SyncProblem::SignInExpired { service };
                let _ = tx.send(Event::SyncProblem(expired));
            }
            drop(job);
        }
        Command::CancelSyncSignIn => {
            services.syncing.cloud.cancel();
            drop(job);
        }
        Command::SyncSignOut { service } => {
            crate::syncing::sign_out(&Ctx::of(services), tx, &service);
            drop(job);
        }
        Command::SyncFolderNow => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::pass(&ctx, &tx).await;
            });
        }
        Command::LeaveSync => {
            crate::syncing::leave(&Ctx::of(services), tx);
            drop(job);
        }
        Command::RemoveSyncDevice { device } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::remove_device(&ctx, &tx, &device).await;
            });
        }
        Command::NewRecoveryKey => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::new_recovery_key(&ctx, &tx).await;
            });
        }
        Command::PairInvite { device_name } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::invite(&ctx, &tx, device_name).await;
            });
        }
        Command::PairStop => {
            crate::syncing::stop_inviting(&Ctx::of(services));
            drop(job);
        }
        Command::PairJoin { device_name, address, code } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::pair(&ctx, &tx, device_name, &address, &code).await;
            });
        }
        Command::SyncWithDevice { device, address } => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::sync_with(&ctx, &tx, &device, &address).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::CopySignIns {
            device,
            copy,
            address,
        }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::handoff::copy_sign_ins(&ctx, &tx, device, &copy, address).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::JoinStoresFrom {
            device,
            stores,
            address,
        }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::store_join::join_from(&ctx, &tx, device, stores, address).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::ChangeSyncFolder { folder }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::change_folder(&ctx, &tx, folder).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::PairWithCode {
            device_name,
            code,
        }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::pair_with_code(&ctx, &tx, device_name, &code).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::RenameDevice { name }) => {
            crate::syncing::rename(&Ctx::of(services), tx, &name);
            drop(job);
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::SetDeviceSync { device, on }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::set_device_sync(&ctx, &tx, &device, on).await;
            });
        }
        Command::SyncGroup(kaeri_model::group::SyncCommand::SetDeviceRemote { device, on }) => {
            let (ctx, tx) = (Ctx::of(services), tx.clone());
            tokio::spawn(async move {
                let _job = job;
                crate::syncing::set_device_remote(&ctx, &tx, &device, on).await;
            });
        }
        Command::ShowRecoveryKey => {
            crate::syncing::reveal_recovery_key(&Ctx::of(services), tx);
            drop(job);
        }
        Command::DismissSyncNote { id } => {
            let _ = services.store.dismiss_sync_note(id);
            let _ = tx.send(Event::SyncStatus(crate::syncing::status(&Ctx::of(services))));
            drop(job);
        }
        Command::Disconnect { tracker } => {
            tracking::disconnect(&services.sync, &tracker, tx);
            tracking::commands::announce_signed_in(&services.store, &services.sync);
            if tracker == "anilist" {
                crate::title_language::signed_out(services, tx);
            }
        }
        Command::SetStatus { id, status } => set_status(id, status, services, tx),
        Command::SetScore { id, score } => set_score(id, score, services, tx),
        Command::ResolveConflict { id, keep_local } => {
            tracking::resolve_conflict(&services.sync, id, keep_local)
        }
        Command::LoadLibrary(segment) => load_library(segment, job, services, tx),

        // Downloads.
        Command::LoadDownloads => downloads::publish_now(&services.store, tx),
        Command::DownloadEpisode { id, episode } => {
            download_episode(id, episode, job, engine, services, tx)
        }
        Command::DownloadMany { id, episodes } => {
            download_many(id, episodes, job, engine, services, tx)
        }
        Command::DownloadStartNow { id } => download_start_now(id, services, tx),
        Command::DownloadPause { id } => download_pause(id, services, tx),
        Command::DownloadCancel { id } => download_cancel(id, services, tx),
        Command::DownloadDelete { id } => download_delete(id, services, tx),
        Command::DownloadClearCompleted => download_clear_completed(services, tx),

        // Settings.
        Command::StepSetting { key, delta } => {
            step_setting(key, delta, engine, services, tx);
            if key == SettingKey::UpdateBeta {
                crate::updates::check_now(
                    &services.paths.cache_dir,
                    engine.config.effective().updates.beta,
                    &services.http,
                    tx,
                );
            }
        }
        Command::SetSetting { key, value } => {
            set_setting(key, value, engine, services, tx);
            // Turning beta updates on or off asks GitHub at once for the other channel's newest.
            if key == SettingKey::UpdateBeta {
                crate::updates::check_now(
                    &services.paths.cache_dir,
                    engine.config.effective().updates.beta,
                    &services.http,
                    tx,
                );
            }
        }
        Command::SetTranslation { translation } => set_translation(translation, engine, tx),
        Command::SetTorrentProxy { proxy } => set_torrent_proxy(proxy, engine, services, tx),
        // Off the loop, with no job: the verdict is the answer.
        Command::CheckTorrentProxy { proxy } => {
            check_torrent_proxy(proxy, engine, services, tx);
        }
        Command::LoadShowSettings { id } => load_show_settings(id, engine, services, tx),
        Command::SetShowSetting { id, key, value } => {
            set_show_setting(id, key, value, engine, services, tx);
        }
        Command::ResetShowSettings { id } => {
            reset_show_settings(id, engine, services, tx);
        }
        Command::SetShowSubtitles { id, translation, subtitles } => {
            set_show_subtitles(id, translation, subtitles, engine, services, tx)
        }

        Command::ClearSkipMarks => crate::skip_marks::clear(services, tx),

        // Off the loop, with no job: covers report back on their own.
        Command::FetchArtwork { url } => artwork::spawn_fetch(
            url,
            services.paths.image_cache(),
            services.http.clone(),
            services.artwork.clone(),
            tx.clone(),
        ),

        // What a native app shows outside itself: notifications, and its widgets. Off the loop,
        // with no job, each answered with one event.
        Command::CheckNewEpisodes => crate::new_episodes::check(engine, services, tx),
        Command::WidgetSnapshot => crate::widgets::snapshot(engine, services, tx),
        Command::UpcomingNotices => crate::upcoming::answer(engine, services, tx),
        // Settings' Check Now, off the loop, with no job: the check says where it stands.
        Command::CheckForUpdates => crate::updates::check_now(
            &services.paths.cache_dir,
            engine.config.effective().updates.beta,
            &services.http,
            tx,
        ),

        // The plugin stores.
        Command::LoadStores => {
            store_task(job, engine, services, tx, StoreTaskContext::load_stores)
        }
        Command::PreviewStore { link } => {
            store_task(job, engine, services, tx, move |context| context.preview_store(link))
        }
        Command::JoinStore { link, label, name } => {
            store_task(job, engine, services, tx, move |context| {
                context.join_store(link, label, name)
            })
        }
        Command::LeaveStore { store } => {
            store_task(job, engine, services, tx, move |context| context.leave_store(store))
        }
        Command::InstallPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.install_plugin(store, plugin)
            })
        }
        Command::RemovePlugin { plugin } => {
            store_task(job, engine, services, tx, move |context| context.remove_plugin(plugin))
        }
        Command::DisablePlugin { id, disabled } => {
            disable_plugin(id, disabled, engine, services, tx)
        }
        Command::CreateInvite { store, name, uses, lasts } => {
            store_task(job, engine, services, tx, move |context| {
                context.create_invite(
                    store,
                    name,
                    uses.unwrap_or_default(),
                    lasts.unwrap_or_default(),
                )
            })
        }
        Command::LoadMembers { store } => {
            store_task(job, engine, services, tx, move |context| context.load_members(store))
        }
        Command::RemoveMember { store, member } => {
            store_task(job, engine, services, tx, move |context| {
                context.remove_member(store, member)
            })
        }
        Command::CancelInvite { store, invite } => {
            store_task(job, engine, services, tx, move |context| {
                context.cancel_invite(store, invite)
            })
        }
        Command::LoadLocalPlugins { store } => {
            store_task(job, engine, services, tx, move |context| {
                context.load_local_plugins(store)
            })
        }
        Command::PublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.publish_plugin(store, plugin)
            })
        }
        Command::UnpublishPlugin { store, plugin } => {
            store_task(job, engine, services, tx, move |context| {
                context.unpublish_plugin(store, plugin)
            })
        }
        Command::OpenAdmin { store } => {
            store_task(job, engine, services, tx, move |context| context.open_admin(store))
        }
        Command::SetOpenJoin { store, open } => {
            store_task(job, engine, services, tx, move |context| {
                context.set_open_join(store, open)
            })
        }
        Command::RemoveAnonymous { store, device } => {
            store_task(job, engine, services, tx, move |context| {
                context.remove_anonymous(store, device)
            })
        }
    }
}

/// The engine side's part in an event on its way to the front end, and what it has to say
/// ahead of the event, for the front end to apply first.
///
/// A finished episode's progress goes to the trackers now, not at the next drain tick: up to a
/// minute later, the pushed progress looked simply absent. And the metadata ladder's notice, when
/// it moved. `tx` is asked for only when there is work to answer on it.
///
/// The actor does this with every event before passing it on.
pub(crate) fn receive(
    update: &Event,
    services: &Services,
    tx: impl FnOnce() -> Outbox,
) -> Vec<Event> {
    if matches!(update, Event::ProgressQueued) {
        let (sync, tx) = (services.sync.clone(), tx());
        tokio::spawn(async move { tracking::drain_once(&sync, &tx).await });
    }
    meta_notice(&services.meta)
}

/// What to say when the metadata ladder moved: once per transition, in both directions, since
/// silently recovering would leave the last thing said untrue. The status rows ride along, so
/// the header chip and the Providers screen agree with the toast without waiting for a refresh.
///
/// The actor says it with the events it passes on.
pub(crate) fn meta_notice(meta: &Meta) -> Vec<Event> {
    let Some(notice) = meta.take_notice() else { return Vec::new() };
    // The ladder words its own transitions; these are the three it makes.
    let msg = match notice.as_str() {
        "anilist is unreachable — fresh data via tenrai for now" => {
            Msg::Store(StoreMsg::NoticeAnilistUnreachable)
        }
        "no metadata source answered — showing what was last cached" => {
            Msg::Store(StoreMsg::NoticeMetadataCached)
        }
        "anilist is back" => Msg::Store(StoreMsg::NoticeAnilistBack),
        _ => Msg::verbatim(notice),
    };
    vec![Event::MetaStatus(data::meta_rows(meta)), Event::Toast(Notice::info(msg))]
}

/// Do what the engine side left itself to do, and return what it then has to say, for the
/// front end to apply at once, before anything sent since. Reached in the message's own place
/// among the events, after everything sent before it.
///
/// The metadata ladder's notice comes first, as on the way of an event. Then the copy of the
/// config the background tasks read is brought up to the one in effect, whatever the message.
///
/// The actor does this with each message in its inbox, in the message's place.
pub(crate) async fn handle_internal(
    message: Internal,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Vec<Event> {
    let mut said = meta_notice(&services.meta);
    if let Ok(mut shared) = engine.shared_config.write() {
        *shared = engine.config.effective().clone();
    }
    match message {
        // What runs on cellular may have changed with it.
        Internal::SettingsSaved => {
            crate::phases::settings_saved(engine, tx);
            crate::downloads::books::resume(engine, services, tx);
        }
        Internal::LetGo => {
            ahead::let_go(engine, services, tx);
            crate::downloads::books::launched(engine, services, tx);
        }
        Internal::Reader { session, wake } => {
            let EngineState { config, reader, .. } = engine;
            let mut cx = reader.cx(config, services, tx);
            reader.wake(&mut cx, session, wake);
        }
        Internal::ReadingSynced => {
            let EngineState { config, reader, .. } = engine;
            let mut cx = reader.cx(config, services, tx);
            reader.synced(&mut cx);
            let lent = crate::downloads::books::Lent::of(services, tx);
            services.book_sources.downloads.sweep(&lent, None);
        }
        Internal::SyncEffects => {
            let effects = services.syncing.take_effects();
            crate::sync::apply_effects(effects, engine, services, tx);
        }
        // Only the order changed: the chain as it stands is put in it, with no rebuild.
        Internal::SourcesReordered => {
            sources::arrange(&services.registry, engine.config.effective());
            crate::titles::refresh::sources_changed(engine, services, tx);
            said.push(Event::Providers(data::provider_rows(
                &services.registry,
                &services.stores,
            )));
            said.push(Event::Demo(crate::demo::state(engine, services)));
        }
        // Awaited rather than spawned: this owns the session and its route, and two
        // rebuilds racing would leave one of their sessions running with nothing holding
        // it. Quick: torrent playback comes up behind the interface, as at launch.
        Internal::SourcesChanged => {
            reload_sources(
                services,
                &mut engine.torrent_tasks,
                &mut engine.torrent_route,
                &engine.shared_config,
                tx,
                &engine.arrivals,
            );
            crate::titles::refresh::sources_changed(engine, services, tx);
            // Passed on, ahead of what the rebuild reports: the screen says it reloads.
            said.push(Event::SourcesChanged);
            said.push(Event::Demo(crate::demo::state(engine, services)));
        }
    }
    said
}
