//! Throwaway QA harness: the real engine with reading on, driven by lines on stdin.
//! Never committed. Run only under a guard HOME.

use std::{
    io::{BufRead, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use kaeri_core::{
    books::{ChapterId, WorkId},
    config::{Config, Paths},
    media::Medium,
};
use kaeri_engine::{engine::Engine, host::Host, options::EngineOptions};
use kaeri_model::{
    Msg,
    book_sources::BookSourceCommand,
    command::Command,
    event::Event,
    playback::PlayerTheme,
    reader::{
        NovelPos, OpenAt, ReaderCommand, ReaderEvent, ReaderSettingKey, ReaderSurface,
        ReaderValue, SettingScope, StripPos, TurnHow,
    },
    reading::{
        DownloadPick, ExportFormat, FeedShelf, LibrarySegmentRead, ReadSearchQuery,
        ReadingCommand, ReadingEvent, WorkTile,
    },
};
use kaeri_net::HttpClient;
use kaeri_store::Store;

static ROWS_SEQ: AtomicU64 = AtomicU64::new(0);

struct QaHost(Arc<Mutex<std::fs::File>>);

impl Host for QaHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        let _ = writeln!(self.0.lock().unwrap(), "HOST open_url refused: {url}");
        Err("the QA harness opens nothing".into())
    }
    fn notify(&self, title: &str, body: &str) {
        let _ = writeln!(self.0.lock().unwrap(), "HOST notify (not posted): {title} | {body}");
    }
}

struct Log {
    start: Instant,
    last_cmd: Mutex<Instant>,
    out: Arc<Mutex<std::fs::File>>,
    full: Mutex<std::fs::File>,
}

impl Log {
    fn line(&self, text: &str) {
        let now = Instant::now();
        let since = now.duration_since(*self.last_cmd.lock().unwrap());
        let line = format!(
            "[{:8.3} +{:6.3}] {text}",
            now.duration_since(self.start).as_secs_f64(),
            since.as_secs_f64()
        );
        let _ = writeln!(self.out.lock().unwrap(), "{line}");
    }
    fn full(&self, text: &str) {
        let now = Instant::now();
        let since = now.duration_since(*self.last_cmd.lock().unwrap());
        let mut t = text.to_owned();
        if t.len() > 12000 {
            let mut cut = 12000;
            while !t.is_char_boundary(cut) {
                cut -= 1;
            }
            t.truncate(cut);
            t.push_str("…");
        }
        let _ = writeln!(
            self.full.lock().unwrap(),
            "[{:8.3} +{:6.3}] {t}",
            now.duration_since(self.start).as_secs_f64(),
            since.as_secs_f64()
        );
    }
}

fn en(m: &Msg) -> String {
    m.english()
}

fn tile(t: &WorkTile) -> String {
    format!(
        "{{{} | {} | {:?} | fmt={} | amount={} | next={} | facts={} | notin={} | prog={:?} | new={} | adult={}}}",
        t.title,
        t.work.0,
        t.medium,
        t.format_label.clone().unwrap_or_default(),
        t.amount.as_ref().map(en).unwrap_or_default(),
        t.next.as_ref().map(en).unwrap_or_default(),
        t.facts.iter().map(en).collect::<Vec<_>>().join("·"),
        t.not_in_sources,
        t.progress,
        t.new_chapter,
        t.adult
    )
}

fn summarize(e: &Event, session: &AtomicU64) -> Option<String> {
    Some(match e {
        Event::Reading(r) => match r {
            ReadingEvent::Search { query, page, tiles, has_more, other_scope, total } => format!(
                "SEARCH q={:?} medium={:?} page={page} n={} more={has_more} other={other_scope:?} total={total:?}\n    {}",
                query.text,
                query.medium,
                tiles.len(),
                tiles.iter().map(tile).collect::<Vec<_>>().join("\n    ")
            ),
            ReadingEvent::SourceSearch(s) => format!(
                "SOURCESEARCH q={:?} anilist={:?} done={} joins={} groups:\n    {}",
                s.query.text,
                s.anilist,
                s.done,
                s.joins.len(),
                s.groups
                    .iter()
                    .map(|g| format!(
                        "{} ({}) {:?} joined={} n={} [{}]",
                        g.source_id,
                        g.source_name,
                        g.state,
                        g.joined,
                        g.tiles.len(),
                        g.tiles.iter().take(5).map(tile).collect::<Vec<_>>().join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join("\n    ")
            ),
            ReadingEvent::Series(s) => format!(
                "SERIES {} {:?} titles={:?} action={:?} fmt={:?} origin={:?} status={:?} ch_total={:?} vol_total={:?} list={:?} edition={:?} rhythm={:?} other={:?} ask={:?}",
                s.work.0,
                s.medium,
                s.titles,
                s.action,
                s.format,
                s.origin,
                s.status,
                s.chapters_total,
                s.volumes_total,
                s.list,
                s.edition,
                s.rhythm.as_ref().map(en),
                s.other_medium.is_some(),
                s.ask_where_stopped
            ),
            ReadingEvent::Chapters { work, edition, volumes, failure } => {
                let rows: Vec<_> = volumes.iter().flat_map(|v| v.chapters.iter()).collect();
                if let Ok(dir) = std::env::var("QA_ROWS") {
                    let n = ROWS_SEQ.fetch_add(1, Ordering::SeqCst);
                    let mut out = String::new();
                    for v in volumes {
                        for c in &v.chapters {
                            out.push_str(&format!(
                                "{}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\tsrc={:?}\tparts={}\tmissing={:?}\tpg={:?}\tog={}\tgone={}\tvers={}\tdl={:?}\t{}\n",
                                c.id.0, c.label, c.number, c.title, c.volume, v.volume, c.state, c.other_source, c.parts, c.missing_parts, c.pages, c.other_group, c.gone_from_source, c.versions, c.download.as_ref().map(|d| format!("{d:?}").chars().take(60).collect::<String>()), c.line.iter().map(en).collect::<Vec<_>>().join("·")
                            ));
                        }
                    }
                    let _ = std::fs::write(format!("{dir}/{}-{n:03}.tsv", work.0.replace(':', "_")), out);
                }
                let nums: Vec<String> = rows
                    .iter()
                    .map(|c| c.number.clone().unwrap_or_else(|| format!("?{}", c.label)))
                    .collect();
                let mut seen = std::collections::HashMap::new();
                for n in &nums {
                    *seen.entry(n.clone()).or_insert(0) += 1;
                }
                let dups: Vec<_> = seen.iter().filter(|(_, c)| **c > 1).map(|(n, c)| format!("{n}x{c}")).collect();
                let states: Vec<String> = rows
                    .iter()
                    .filter(|c| !matches!(c.state, kaeri_model::reading::ChapterReadState::Unread))
                    .map(|c| format!("{}={:?}", c.number.clone().unwrap_or_default(), c.state))
                    .take(12)
                    .collect();
                format!(
                    "CHAPTERS {} edition={:?} failure={:?} rows={} volumes={} vols=[{}] dups={dups:?} labels_sample=[{}] versions>1={} vlabels={:?} pages_some={} new={} dl={} gone={} states={states:?}\n    nums={}",
                    work.0,
                    edition,
                    failure,
                    rows.len(),
                    volumes.len(),
                    volumes
                        .iter()
                        .map(|v| format!("{:?}/{:?}:{}{}", v.volume, v.title, v.chapters.len(), if v.all_read { "R" } else { "" }))
                        .collect::<Vec<_>>()
                        .join(" "),
                    rows.iter().take(3).chain(rows.iter().rev().take(2)).map(|c| format!("{:?}|{:?}|{:?}|{}", c.label, c.title, c.volume, c.line.iter().map(en).collect::<Vec<_>>().join("·"))).collect::<Vec<_>>().join(" ;; "),
                    rows.iter().filter(|c| c.versions > 1).count(),
                    rows.iter().filter_map(|c| c.version_label.clone()).collect::<std::collections::BTreeSet<_>>(),
                    rows.iter().filter(|c| c.pages.is_some()).count(),
                    rows.iter().filter(|c| c.new_chapter).count(),
                    rows.iter().filter(|c| c.download.is_some()).count(),
                    rows.iter().filter(|c| c.gone_from_source).count(),
                    nums.join(",")
                )
            }
            ReadingEvent::Library { segment, tiles } => format!(
                "LIBRARY {segment:?} n={}\n    {}",
                tiles.len(),
                tiles.iter().map(tile).collect::<Vec<_>>().join("\n    ")
            ),
            ReadingEvent::Failed { about, failure } => {
                format!("FAILED about={about:?} failure={failure:?} says={:?}", en(&failure.msg()))
            }
            ReadingEvent::Feed(f) => format!(
                "FEED on={} addresses={:?} port={} password_set={} serving={} shares={:?} clients={} pause={:?}",
                f.on, f.addresses, f.port, f.password_set, f.serving, f.shares, f.clients.len(), f.pause
            ),
            other => {
                let d = format!("{other:?}");
                format!("READING {}", if d.len() > 3000 { format!("{}…", &d[..d.char_indices().nth(3000).map(|(i, _)| i).unwrap_or(d.len())]) } else { d })
            }
        },
        Event::Reader(r) => match r {
            ReaderEvent::Opened(v) => {
                session.store(v.session, Ordering::SeqCst);
                format!(
                    "OPENED session={} {} {:?} ch={{id={} label={:?} num={:?} pages={:?} state={:?} line={}}} mode={:?} dir={:?} start={:?} next={:?} prev={:?}",
                    v.session,
                    v.title,
                    v.medium,
                    v.chapter.id.0,
                    v.chapter.label,
                    v.chapter.number,
                    v.chapter.pages,
                    v.chapter.state,
                    v.chapter.line.iter().map(en).collect::<Vec<_>>().join("·"),
                    v.mode,
                    v.direction,
                    v.start,
                    v.next.as_ref().map(|c| (&c.id.0, &c.label)),
                    v.previous.as_ref().map(|c| (&c.id.0, &c.label))
                )
            }
            ReaderEvent::Leaves { session, chapter, leaves } => format!(
                "LEAVES s={session} ch={} n={} end_pages={} parts={:?}",
                chapter.0,
                leaves.len(),
                leaves.iter().filter(|l| l.end_page).count(),
                leaves
                    .iter()
                    .map(|l| l
                        .parts
                        .iter()
                        .map(|p| format!(
                            "{}{:?}{}",
                            p.page,
                            p.kind,
                            p.file.as_ref().map(|f| format!("@{}x{}", f.width, f.height)).unwrap_or_default()
                        ))
                        .collect::<Vec<_>>()
                        .join("+"))
                    .collect::<Vec<_>>()
            ),
            ReaderEvent::PageReady { session, chapter, page, half, file } => format!(
                "PAGE s={session} ch={} p={page} half={half:?} {}x{} bytes={} enc={:?} crop={:?} colour={}",
                chapter.0, file.width, file.height, file.bytes, file.encoding, file.crop, file.colour
            ),
            ReaderEvent::NovelOpen(n) => format!(
                "NOVELOPEN s={} ch={} entry={} wm={:?} prog={:?} restore={:?} fixed={} spine={:?} toc={} count_key={} counted={:?} wpm={} render_dir={:?}",
                n.session,
                n.chapter.0,
                n.entry,
                n.writing_mode,
                n.progression,
                n.restore,
                n.fixed_layout,
                n.spine.iter().map(|s| format!("{}:{}w", s.url, s.words)).collect::<Vec<_>>(),
                n.toc.len(),
                n.count_key,
                n.counted,
                n.words_per_minute,
                n.render_dir
            ),
            ReaderEvent::EndPage { session, next, why_none, next_source, skip_to, extras } => format!(
                "ENDPAGE s={session} next={:?} why_none={why_none:?} next_source={next_source:?} skip_to={:?} extras={extras}",
                next.as_ref().map(|c| (&c.id.0, &c.label, c.state)),
                skip_to.as_ref().map(|c| (&c.id.0, &c.label, c.state))
            ),
            ReaderEvent::Moved { session, to, from_device, back } => format!(
                "MOVED s={session} to={}:{:?} from={from_device:?} back={}:{:?}",
                to.chapter.0, to.page, back.chapter.0, back.page
            ),
            ReaderEvent::Notice(n) => format!("NOTICE {:?} action={:?} ms={}", en(&n.msg), n.action, n.lasts_ms),
            other => {
                let d = format!("{other:?}");
                format!("READER {}", if d.len() > 2500 { format!("{}…", &d[..d.char_indices().nth(2500).map(|(i, _)| i).unwrap_or(d.len())]) } else { d })
            }
        },
        Event::BookSources(b) => {
            let d = format!("{b:?}");
            format!("BOOKSRC {}", d.chars().take(4000).collect::<String>())
        }
        Event::JobStarted { .. } | Event::JobDone(_) | Event::JobNote { .. } => {
            let d = format!("{e:?}");
            format!("JOB {}", d.chars().take(300).collect::<String>())
        }
        other => {
            let d = format!("{other:?}");
            let name: String = d.chars().take(400).collect();
            format!("EV {name}")
        }
    })
}

fn query(words: &[&str]) -> ReadSearchQuery {
    let mut q = ReadSearchQuery {
        text: String::new(),
        medium: None,
        formats: vec![],
        origins: vec![],
        status: None,
        genres: vec![],
        exclude_genres: vec![],
        sort: None,
        started: None,
        sources_only: false,
        source_scope: None,
    };
    let mut text = Vec::new();
    let mut in_text = false;
    for w in words {
        if in_text {
            text.push(*w);
            continue;
        }
        if *w == "--" {
            in_text = true;
            continue;
        }
        let (k, v) = w.split_once('=').unwrap_or((w, ""));
        let list = || v.split(',').filter(|s| !s.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        match k {
            "m" => q.medium = Medium::parse(v),
            "f" => q.formats = list(),
            "o" => q.origins = list(),
            "s" => q.status = Some(v.to_owned()),
            "g" => q.genres = list(),
            "x" => q.exclude_genres = list(),
            "sort" => q.sort = Some(v.to_owned()),
            "started" => q.started = Some(v.to_owned()),
            "src" => q.sources_only = true,
            "in" => q.source_scope = Some(list()),
            _ => text.push(*w),
        }
    }
    q.text = text.join(" ");
    q
}

fn pick(words: &[&str]) -> DownloadPick {
    match words.first().copied() {
        Some("next") => DownloadPick::NextUnread { n: words.get(1).and_then(|n| n.parse().ok()).unwrap_or(1) },
        Some("unread") => DownloadPick::Unread,
        Some("vol") => DownloadPick::Volume { volume: words.get(1).unwrap_or(&"1").to_string() },
        _ => DownloadPick::Chapters {
            ids: words.iter().skip(1).map(|s| ChapterId::new(*s)).collect(),
        },
    }
}

fn segment(s: &str) -> LibrarySegmentRead {
    LibrarySegmentRead::parse(s).unwrap_or(LibrarySegmentRead::Reading)
}

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(run());
}

async fn run() {
    let home = std::env::var("HOME").unwrap();
    assert!(!home.starts_with("/Users/johan") || home.contains("qa-home"), "guard HOME required");
    let log_path = std::env::var("QA_LOG").expect("QA_LOG");
    let out = Arc::new(Mutex::new(
        std::fs::OpenOptions::new().create(true).append(true).open(&log_path).unwrap(),
    ));
    let full = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{log_path}.full"))
        .unwrap();
    let log = Arc::new(Log {
        start: Instant::now(),
        last_cmd: Mutex::new(Instant::now()),
        out: out.clone(),
        full: Mutex::new(full),
    });

    let paths = Paths::resolve().unwrap();
    assert!(paths.data_dir.starts_with(&home), "data dir outside the guard");
    {
        use tracing_subscriber::{EnvFilter, fmt};
        let dir = paths.cache_dir.join("logs");
        std::fs::create_dir_all(&dir).unwrap();
        let appender = tracing_appender::rolling::never(&dir, "qa.log");
        let _ = fmt()
            .with_writer(appender)
            .with_ansi(false)
            .with_env_filter(EnvFilter::try_from_env("KAERI_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
            .try_init();
    }
    let config = Config::read(&paths).expect("config");
    assert!(!config.meta.adult, "adult must be off");
    kaeri_i18n::init(config.language.as_deref());
    let saved = Config::load(&paths).unwrap_or_default();
    let http = HttpClient::new(&config.network).unwrap();
    let (store, _aside) = Store::open_or_set_aside(paths.database()).unwrap();
    let theme = PlayerTheme {
        text: "#ffffff".into(),
        dim: "#888888".into(),
        obi: "#222222".into(),
        state: "#44aa44".into(),
        alert: "#cc3333".into(),
        ground: "#000000".into(),
        font: "sans".into(),
    };
    let mut options = EngineOptions::new(paths.clone(), config.clone(), http, store, theme);
    options.saved = Some(saved);
    options.reading = true;
    if std::env::var("QA_QUIET").is_ok() {
        options.background = kaeri_engine::options::Background::none();
    }
    let host = Arc::new(QaHost(out.clone()));
    let t0 = Instant::now();
    let (engine, mut events) = Engine::start(options, host).await.expect("engine");
    log.line(&format!("ENGINE started in {:?}", t0.elapsed()));
    engine.send(Command::NetworkPath { cellular: false, constrained: false, offline: false });
    for e in events.launch().await {
        log.full(&format!("LAUNCH {e:?}"));
    }
    let session = Arc::new(AtomicU64::new(0));
    {
        let log = log.clone();
        let session = session.clone();
        tokio::spawn(async move {
            while let Some(e) = events.recv().await {
                if let Some(s) = summarize(&e, &session) {
                    log.line(&s);
                }
                log.full(&format!("{e:?}"));
            }
            log.line("EVENT STREAM ENDED");
        });
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let surface = ReaderSurface {
        width_px: 1600,
        height_px: 1000,
        scale: 2.0,
        eink: false,
        two_pages: false,
        dark: false,
    };
    let mut surface = surface;
    while let Some(line) = rx.recv().await {
        let line = line.trim().to_owned();
        if line.is_empty() {
            continue;
        }
        *log.last_cmd.lock().unwrap() = Instant::now();
        log.line(&format!(">>> {line}"));
        log.full(&format!(">>> {line}"));
        let w: Vec<&str> = line.split_whitespace().collect();
        let s = session.load(Ordering::SeqCst);
        let work = |i: usize| WorkId::new(w.get(i).copied().unwrap_or(""));
        let ch = |i: usize| ChapterId::new(w.get(i).copied().unwrap_or(""));
        let num = |i: usize| w.get(i).and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
        let rc = |c: ReadingCommand| Some(Command::Reading(c));
        let dc = |c: ReaderCommand| Some(Command::Reader(c));
        let cmd: Option<Command> = match w[0] {
            "#" => None,
            "quit" => break,
            "sleep" => {
                tokio::time::sleep(Duration::from_millis(num(1).into())).await;
                None
            }
            "offline" => Some(Command::NetworkPath { cellular: false, constrained: false, offline: true }),
            "online" => Some(Command::NetworkPath { cellular: false, constrained: false, offline: false }),
            "home" => rc(ReadingCommand::LoadHome { refresh: w.get(1) == Some(&"refresh") }),
            "updates" => rc(ReadingCommand::LoadUpdates),
            "check" => rc(ReadingCommand::CheckUpdates),
            "dlnew" => rc(ReadingCommand::DownloadNew),
            "lib" => rc(ReadingCommand::LoadLibrary { segment: segment(w.get(1).unwrap_or(&"reading")) }),
            "search" => rc(ReadingCommand::Search { query: query(&w[1..]) }),
            "more" => rc(ReadingCommand::SearchMore { page: num(1), query: query(&w[2..]) }),
            "retrysrc" => rc(ReadingCommand::RetrySourceSearch { source_id: w[1].into(), query: query(&w[2..]) }),
            "series" => rc(ReadingCommand::LoadSeries { work: work(1) }),
            "chapters" => rc(ReadingCommand::LoadChapters { work: work(1), edition: w.get(2).map(|s| s.to_string()) }),
            "versions" => rc(ReadingCommand::LoadVersions { work: work(1), chapter: ch(2) }),
            "edition" => rc(ReadingCommand::ChooseEdition {
                work: work(1),
                source_id: w[2].into(),
                version_label: w.get(3).filter(|s| **s != "-").map(|s| s.replace('_', " ")),
                language: w.get(4).filter(|s| **s != "-").map(|s| s.to_string()),
            }),
            "mark" => rc(ReadingCommand::MarkRead {
                work: work(1),
                read: w.get(2) == Some(&"true"),
                chapters: w.iter().skip(3).map(|c| ChapterId::new(*c)).collect(),
            }),
            "upto" => rc(ReadingCommand::MarkReadUpTo { work: work(1), chapter: ch(2) }),
            "again" => rc(ReadingCommand::ReadAgain { work: work(1) }),
            "undoagain" => rc(ReadingCommand::UndoReadAgain { work: work(1) }),
            "bookmark" => rc(ReadingCommand::Bookmark { work: work(1), chapter: ch(2), on: w.get(3) == Some(&"on") }),
            "status" => rc(ReadingCommand::SetStatus { work: work(1), status: segment(w.get(2).unwrap_or(&"reading")) }),
            "tracking" => rc(ReadingCommand::LoadTracking { work: work(1) }),
            "dl" => rc(ReadingCommand::Download { work: work(1), pick: pick(&w[2..]) }),
            "est" => rc(ReadingCommand::EstimateDownload { work: work(1), pick: pick(&w[2..]) }),
            "dlcancel" => rc(ReadingCommand::DownloadCancel { work: work(1), chapters: w.iter().skip(2).map(|c| ChapterId::new(*c)).collect() }),
            "dldel" => rc(ReadingCommand::DownloadDelete { work: work(1), chapters: w.iter().skip(2).map(|c| ChapterId::new(*c)).collect() }),
            "dldelread" => rc(ReadingCommand::DownloadDeleteRead { work: work(1) }),
            "downloads" => rc(ReadingCommand::LoadDownloads),
            "targets" => rc(ReadingCommand::LoadExportTargets),
            "prepare" => rc(ReadingCommand::Prepare {
                work: work(1),
                target: w[2].into(),
                format: match w[3] {
                    "kepub" => ExportFormat::Kepub,
                    "epub" => ExportFormat::Epub,
                    "cbz" => ExportFormat::Cbz,
                    _ => ExportFormat::FixedEpub,
                },
                pick: pick(&w[4..]),
            }),
            "sendtarget" => rc(ReadingCommand::SetSendTarget { target: w[1].into() }),
            "feed" => rc(ReadingCommand::LoadFeed),
            "setfeed" => rc(ReadingCommand::SetFeed {
                on: w.get(1) == Some(&"on"),
                password: w.get(2).map(|p| kaeri_model::stores::Secret(p.to_string())),
            }),
            "share" => rc(ReadingCommand::SetFeedShare {
                shelf: match w[1] {
                    "manga" => FeedShelf::MangaFolders,
                    "novels" => FeedShelf::NovelFolders,
                    _ => FeedShelf::Downloads,
                },
                on: w.get(2) == Some(&"on"),
            }),
            "srcload" => Some(Command::BookSources(BookSourceCommand::Load)),
            "forwork" => Some(Command::BookSources(BookSourceCommand::LoadForWork { work: work(1) })),
            "srcsearch" => Some(Command::BookSources(BookSourceCommand::SearchSourceForWork {
                work: work(1),
                source_id: w[2].into(),
                query: w[3..].join(" "),
            })),
            "srcchoose" => Some(Command::BookSources(BookSourceCommand::ChooseSourceSeries {
                work: work(1),
                source_id: w[2].into(),
                series_ref: w[3].into(),
            })),
            "srcoff" => Some(Command::BookSources(BookSourceCommand::SetOff { id: w[1].into(), off: w.get(2) == Some(&"off") })),
            "srcorder" => Some(Command::BookSources(BookSourceCommand::SetOrder { ids: w[1..].iter().map(|s| s.to_string()).collect() })),
            "surface" => {
                surface.width_px = num(1);
                surface.height_px = num(2);
                surface.two_pages = w.get(3) == Some(&"two");
                dc(ReaderCommand::Surface { session: s, surface })
            }
            "open" => dc(ReaderCommand::Open {
                work: work(1),
                chapter: w.get(2).filter(|c| **c != "-").map(|c| ChapterId::new(*c)),
                at: match w.get(3).copied() {
                    Some("start") => OpenAt::Start,
                    Some(p) if p.starts_with('p') => OpenAt::Page { page: p[1..].parse().unwrap_or(0) },
                    _ => OpenAt::Resume,
                },
                surface,
            }),
            "shown" => dc(ReaderCommand::Shown { session: s, chapter: ch(1), leaf: num(2), how: TurnHow::Key }),
            "strip" => dc(ReaderCommand::StripAt {
                session: s,
                chapter: ch(1),
                at: StripPos { image: num(2), offset: w.get(3).and_then(|o| o.parse().ok()).unwrap_or(0.0) },
                foot_visible: w.get(4) == Some(&"foot"),
            }),
            "need" => dc(ReaderCommand::NeedPage { session: s, chapter: ch(1), page: num(2) }),
            "retrypage" => dc(ReaderCommand::Retry { session: s, chapter: ch(1), page: w.get(2).and_then(|p| p.parse().ok()) }),
            "och" => dc(ReaderCommand::OpenChapter { session: s, chapter: ch(1) }),
            "backto" => dc(ReaderCommand::BackTo { session: s }),
            "startover" => dc(ReaderCommand::StartOver { session: s }),
            "goback" => dc(ReaderCommand::GoBack { session: s }),
            "close" => dc(ReaderCommand::Close { session: s }),
            "novelat" => dc(ReaderCommand::NovelAt {
                session: s,
                chapter: ch(1),
                pos: NovelPos::decode(w.get(2).unwrap_or(&"0//0")).unwrap_or(NovelPos { spine: 0, path: vec![], offset: 0 }),
                page: num(3),
                pages: num(4),
                how: TurnHow::Key,
            }),
            "counted" => dc(ReaderCommand::NovelCounted {
                session: s,
                key: w[1].into(),
                chapter_pages: w.get(2).unwrap_or(&"").split(',').filter_map(|n| n.parse().ok()).collect(),
            }),
            "settings" => dc(ReaderCommand::LoadSettings {
                medium: Medium::parse(w.get(1).unwrap_or(&"manga")).unwrap_or(Medium::Manga),
                work: w.get(2).map(|x| WorkId::new(*x)),
            }),
            // set <medium> <key> <scope> <value> [work]
            "set" => dc(ReaderCommand::SetSetting {
                session: Some(s).filter(|s| *s != 0),
                medium: Medium::parse(w[1]).unwrap_or(Medium::Manga),
                key: ReaderSettingKey::from_name(w[2]).expect("setting key"),
                scope: match w[3] {
                    "series" => SettingScope::Series,
                    "here" => SettingScope::SeriesHere,
                    "device" => SettingScope::Device,
                    _ => SettingScope::Medium,
                },
                value: ReaderValue::decode(w[4]),
                work: w.get(5).map(|x| WorkId::new(*x)),
            }),
            "walk" => {
                // walk <chapter> <from> <to> <ms>
                let (from, to, ms) = (num(2), num(3), num(4).max(50));
                for leaf in from..=to {
                    engine.send(Command::Reader(ReaderCommand::Shown {
                        session: session.load(Ordering::SeqCst),
                        chapter: ch(1),
                        leaf,
                        how: TurnHow::Key,
                    }));
                    tokio::time::sleep(Duration::from_millis(ms.into())).await;
                }
                None
            }
            "stripwalk" => {
                // stripwalk <chapter> <from> <to> <ms>
                let (from, to, ms) = (num(2), num(3), num(4).max(50));
                for image in from..=to {
                    engine.send(Command::Reader(ReaderCommand::StripAt {
                        session: session.load(Ordering::SeqCst),
                        chapter: ch(1),
                        at: StripPos { image, offset: 0.0 },
                        foot_visible: image == to && w.get(5) == Some(&"foot"),
                    }));
                    tokio::time::sleep(Duration::from_millis(ms.into())).await;
                }
                None
            }
            "res" => {
                let url = w.get(1).copied().unwrap_or("");
                match engine.book_resource(url) {
                    None => log.line(&format!("RES {url} -> None")),
                    Some(r) => {
                        let (len, text) = match &r.body {
                            kaeri_model::reader::BookBody::Bytes { data } => {
                                (data.len(), String::from_utf8_lossy(data).into_owned())
                            }
                            kaeri_model::reader::BookBody::File { path } => {
                                let d = std::fs::read(path).unwrap_or_default();
                                (d.len(), String::from_utf8_lossy(&d).into_owned())
                            }
                        };
                        let count = |pat: &str| text.matches(pat).count();
                        let plain: String = {
                            let mut out = String::new();
                            let mut tag = false;
                            for c in text.chars() {
                                match c {
                                    '<' => tag = true,
                                    '>' => tag = false,
                                    _ if !tag => out.push(c),
                                    _ => {}
                                }
                            }
                            out
                        };
                        let cjk = plain.chars().filter(|c| ('\u{3040}'..='\u{9fff}').contains(c)).count();
                        log.line(&format!(
                            "RES {url} status={} mime={} bytes={len} chars_text={} cjk={cjk} <p>={} <ruby>={} <rt>={} <img>={} <image>={} <h1>={} <h2>={} <script>={} <link>={} <style>={} replacement_chars={} hrefs={:?}",
                            r.status,
                            r.mime,
                            plain.trim().chars().count(),
                            count("<p"),
                            count("<ruby"),
                            count("<rt"),
                            count("<img"),
                            count("<image"),
                            count("<h1"),
                            count("<h2"),
                            count("<script"),
                            count("<link"),
                            count("<style"),
                            count("\u{fffd}"),
                            text.match_indices("src=\"").map(|(i, _)| text[i..].chars().take(120).collect::<String>()).take(6).collect::<Vec<_>>()
                        ));
                        if let Ok(dir) = std::env::var("QA_DUMP") {
                            let name = url.replace(['/', ':'], "_");
                            let _ = std::fs::write(format!("{dir}/{name}"), text.as_bytes());
                        }
                    }
                }
                None
            }
            other => {
                log.line(&format!("?? unknown command {other}"));
                None
            }
        };
        if let Some(c) = cmd {
            let job = engine.send(c);
            log.full(&format!("sent job={job:?}"));
        }
    }
    log.line("QUIT: closing");
    drop(engine);
    tokio::time::sleep(Duration::from_millis(500)).await;
}
