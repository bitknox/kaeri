//! Bug hunt drives for the reading side, round 2 (hunt worktree only, never committed). Each
//! `hunt-*` shot runs a scripted stress and logs `HUNT` lines: the process's RSS, live heap,
//! threads, file handles, WebKit processes, the reader's own state and where the keyboard is.

use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_core::{
    books::{
        BookHints, BookSourceKind, BookSourceManifest, ChapterContent, MadePage, PageRef,
        SeriesDetail, SeriesHit, SourceChapter,
    },
    error::ProviderError,
    media::Medium,
    traits::{BookSource, PageError},
};
use kaeri_model::input::{Direction, ReaderPad};

use crate::{pages::Dest, window::Shell};

const MANGA: &str = "a:900301";
const NOVEL: &str = "a:900302";
const STRIP: &str = "a:900303";
/// The hunt source's long manga: 1200 chapters, decimals and parts, and pages that misbehave.
const BIG: &str = "a:900304";
/// The hunt source's strip: 120 tall images in chapter 1.
const TALL: &str = "a:900305";
/// The hunt source's novel: ruby text.
const RUBY: &str = "a:900306";

/// Where shots and requests go: $HUNT_OUT, or /out/hunt as in the box.
fn out_dir() -> String {
    std::env::var("HUNT_OUT").unwrap_or_else(|_| "/out/hunt".into())
}

static CHAPTER_ASKS: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------------------------
// A made-up book source.

pub struct Tide {
    manifest: BookSourceManifest,
}

pub fn source() -> Arc<dyn BookSource> {
    Arc::new(Tide {
        manifest: BookSourceManifest {
            id: "hunt-tidewater".into(),
            name: "Tidewater Shelf".into(),
            version: "1".into(),
            kind: BookSourceKind::Native,
            media: vec![Medium::Manga, Medium::Novel],
            languages: vec!["en".into()],
            allowed_hosts: vec!["tide.example".into()],
            capabilities: Vec::new(),
        },
    })
}

/// Matches for the hunt source's series, as if confirmed once: the long manga, the strip and the
/// novel.
pub fn seed(store: &kaeri_store::Store, now: i64) {
    for (series, work) in [("tb", BIG), ("mi", TALL), ("sr", RUBY)] {
        if let Err(e) =
            store.set_book_match("hunt-tidewater", series, work, 100, true, false, now)
        {
            tracing::warn!("HUNT seed {series}: {e}");
        }
    }
}

fn page(series: &str, chapter: &str, i: u32, w: u32, h: u32) -> PageRef {
    let mut p = PageRef::new(format!("https://tide.example/{series}/{chapter}/{i}"));
    p.made = Some(MadePage::new(series, chapter));
    p.width = Some(w);
    p.height = Some(h);
    p
}

fn png(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut img = image::RgbImage::new(w, h);
    let base = [
        (seed * 37 % 200) as u8 + 30,
        (seed * 91 % 200) as u8 + 30,
        (seed * 53 % 200) as u8 + 30,
    ];
    for (x, y, px) in img.enumerate_pixels_mut() {
        let band = ((y / 40) + (x / 40)) % 2 == 0;
        *px = if band { image::Rgb(base) } else { image::Rgb([base[0] / 2, base[1] / 2, 240]) };
    }
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = img.write_to(&mut out, image::ImageFormat::Png);
    out.into_inner()
}

#[async_trait::async_trait]
impl BookSource for Tide {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _: &BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError> {
        let q = query.to_lowercase();
        tracing::info!(query, "HUNT tide search");
        if q.contains("hang") {
            std::future::pending::<()>().await;
        }
        if q.contains("fail") {
            return Err(ProviderError::Other("the shelf is closed".into()));
        }
        if q.contains("slow") {
            tokio::time::sleep(Duration::from_secs(4)).await;
        }
        if q.contains("tidebell") || q.contains("shiokane") {
            return Ok(vec![SeriesHit::new("tb", "Tidebell Cartographer")]);
        }
        if q.contains("moss") || q.contains("tai yu") {
            return Ok(vec![SeriesHit::new("mi", "Moss and Iron")]);
        }
        if q.contains("salt road") || q.contains("shio no") {
            return Ok(vec![SeriesHit::new("sr", "Salt Road Letters")]);
        }
        if q.contains("kettle") || q.contains("fuyugitsune") {
            return Ok(vec![SeriesHit::new("kf", "A Kettle for the Winter Fox")]);
        }
        let long = "The Exceedingly Long Chronicle of the Lantern Ferry and Everyone Who Ever \
                    Waited on Its Pier Through Seven Winters of Fog";
        Ok(vec![
            SeriesHit::new("t-1", format!("{query} {long}")),
            SeriesHit::new("t-2", "灯籠の渡し舟と霧の七つの冬の長い長い物語 第二部 完全版"),
            SeriesHit::new("t-3", format!("{query} the third")),
        ])
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        let title = match id {
            "tb" => "Tidebell Cartographer",
            "mi" => "Moss and Iron",
            "sr" => "Salt Road Letters",
            "kf" => "A Kettle for the Winter Fox",
            other => other,
        };
        Ok(SeriesDetail::new(id, title))
    }

    async fn chapters(
        &self,
        id: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        let ask = CHAPTER_ASKS.fetch_add(1, Ordering::SeqCst);
        tracing::info!("HUNT tide chapters {id} ask {ask}");
        if std::env::var("HUNT_CHAPTERS_SLOW").is_ok() {
            tokio::time::sleep(Duration::from_secs(6)).await;
        }
        let mut out = Vec::new();
        match id {
            "tb" => {
                // 1200 chapters and one more on every visit; chapter 7 comes and goes; decimal
                // and part chapters; two versions of chapter 20.
                let last = 1200 + ask;
                for n in 1..=last {
                    if n == 7 && ask % 2 == 1 {
                        continue;
                    }
                    let mut c = SourceChapter::new(format!("c-{n}"), format!("{n}"));
                    c.language = Some("en".into());
                    c.page_count = Some(12);
                    out.push(c);
                    if n % 100 == 10 {
                        let mut c = SourceChapter::new(format!("c-{n}.5"), format!("{n}.5"));
                        c.language = Some("en".into());
                        out.push(c);
                    }
                }
                for label in ["50-51", "Extra", "Part 2"] {
                    let mut c = SourceChapter::new(format!("c-{label}"), label);
                    c.language = Some("en".into());
                    out.push(c);
                }
                let mut c = SourceChapter::new("c-20b", "20");
                c.version_label = Some("Second Crew".into());
                c.language = Some("en".into());
                out.push(c);
            }
            "mi" => {
                for n in 1..=30 {
                    out.push(SourceChapter::new(format!("c-{n}"), format!("{n}")));
                }
            }
            "sr" => {
                for n in 1..=12 {
                    out.push(SourceChapter::new(format!("c-{n}"), format!("{n}")));
                }
            }
            "kf" => out.push(SourceChapter::new("c-one", "Oneshot")),
            _ => {}
        }
        Ok(out)
    }

    async fn content(
        &self,
        series: &str,
        chapter: &str,
    ) -> Result<ChapterContent, ProviderError> {
        tracing::info!("HUNT tide content {series} {chapter}");
        if std::env::var("HUNT_CONTENT_SLOW").is_ok() {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        match series {
            "sr" => {
                let mut body = String::from(
                    "<h1>Letter</h1><p>Pictures: quick, slow, missing, hanging, broken.</p>\
                     <img src=\"https://tide.example/sr/pic/0\" alt=\"quick\"/>\
                     <img src=\"https://tide.example/sr/pic/1\" alt=\"slow\"/>\
                     <img src=\"https://tide.example/sr/pic/2\" alt=\"missing\"/>\
                     <img src=\"https://tide.example/sr/pic/3\" alt=\"hanging\"/>\
                     <img src=\"https://tide.example/sr/pic/4\" alt=\"broken\"/>",
                );
                for i in 0..60 {
                    body.push_str(&format!(
                        "<p>{i}. The <ruby>塩<rt>しお</rt></ruby> road ran past the \
                         <ruby>灯台<rt>とうだい</rt></ruby> and on to the sea, where the \
                         letters waited in a tin box under the step.</p>"
                    ));
                }
                Ok(ChapterContent::Text(body))
            }
            "mi" => {
                let n = if chapter == "c-1" { 120 } else { 20 };
                Ok(ChapterContent::Images(
                    (0..n).map(|i| page(series, chapter, i, 800, 2400)).collect(),
                ))
            }
            _ => {
                let n = match chapter {
                    "c-1" => 150,
                    "c-2" => 20,
                    _ => 12,
                };
                Ok(ChapterContent::Images(
                    (0..n).map(|i| page(series, chapter, i, 600, 900)).collect(),
                ))
            }
        }
    }

    async fn fetch_page(&self, page: &PageRef) -> Result<Vec<u8>, PageError> {
        let mut parts = page.url.rsplit('/');
        let i: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let chapter = parts.next().unwrap_or("").to_owned();
        let series = parts.next().unwrap_or("").to_owned();
        if let Ok(v) = std::env::var("HUNT_PAGES_SLOW") {
            tokio::time::sleep(Duration::from_millis(v.parse().unwrap_or(1500))).await;
        }
        let seed = i + chapter.len() as u32 * 7;
        if series == "mi" {
            return Ok(png(800, 2400, seed));
        }
        if series == "sr" {
            tracing::info!("HUNT tide picture {i}");
            match i {
                1 => tokio::time::sleep(Duration::from_secs(6)).await,
                2 => return Err(PageError::Status(404)),
                3 => {
                    std::future::pending::<()>().await;
                }
                4 => return Ok(b"not a picture".to_vec()),
                _ => {}
            }
            return Ok(png(400, 300, seed));
        }
        if chapter == "c-2" {
            match i {
                2 => return Ok(b"this is not an image at all".to_vec()),
                4 => {
                    std::future::pending::<()>().await;
                }
                6 => tokio::time::sleep(Duration::from_secs(6)).await,
                8 => return Err(PageError::Status(404)),
                10 => return Ok(png(2000, 16000, seed)),
                12 => return Ok(png(1, 1, seed)),
                14 => return Ok(png(10, 4000, seed)),
                16 => return Ok(png(9000, 600, seed)),
                _ => {}
            }
        }
        Ok(png(600, 900, seed))
    }
}

// ---------------------------------------------------------------------------------------------
// Probes

fn status_field(status: &str, name: &str) -> String {
    status
        .lines()
        .find(|l| l.starts_with(name))
        .map(|l| l[name.len()..].trim().to_owned())
        .unwrap_or_default()
}

fn heap_kb() -> usize {
    // SAFETY: mallinfo2 reads the allocator's counters.
    unsafe { libc::mallinfo2().uordblks / 1024 }
}

/// The process, its file handles and the WebKit processes beside it.
pub fn proc_facts() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let fds = std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0);
    let mut webkit = 0;
    let mut webkit_kb = 0u64;
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for entry in dir.flatten() {
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            if comm.starts_with("WebKit") {
                webkit += 1;
                let s =
                    std::fs::read_to_string(entry.path().join("status")).unwrap_or_default();
                webkit_kb += status_field(&s, "VmRSS:")
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
                    .unwrap_or(0);
            }
        }
    }
    format!(
        "rss={} heap_kb={} threads={} fds={} webkit_procs={} webkit_rss_kb={}",
        status_field(&status, "VmRSS:").replace(' ', ""),
        heap_kb(),
        status_field(&status, "Threads:"),
        fds,
        webkit,
        webkit_kb
    )
}

/// A widget in a few words: its type, its css name, its label or tooltip, its action.
pub fn describe(widget: &gtk::Widget) -> String {
    let mut out = format!("{}", widget.type_().name());
    let name = widget.widget_name();
    if !name.is_empty() && name != widget.type_().name() {
        out.push_str(&format!("#{name}"));
    }
    let classes = widget.css_classes();
    if !classes.is_empty() {
        out.push_str(&format!(
            ".{}",
            classes.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(".")
        ));
    }
    let mut text = Vec::new();
    labels(widget, &mut text);
    if let Some(button) = widget.downcast_ref::<gtk::Button>()
        && let Some(label) = button.label()
    {
        text.insert(0, label.to_string());
    }
    if let Some(row) = widget.downcast_ref::<adw::PreferencesRow>() {
        text.insert(0, row.title().to_string());
    }
    if let Some(tip) = widget.tooltip_text() {
        text.push(format!("tip:{tip}"));
    }
    if let Some(a) = widget.dynamic_cast_ref::<gtk::Actionable>()
        && let Some(name) = a.action_name()
    {
        out.push_str(&format!(" action={name}"));
        if let Some(t) = a.action_target_value() {
            out.push_str(&format!("({t})"));
        }
    }
    if text.is_empty() {
        let mut up = widget.parent();
        let mut chain = Vec::new();
        while let Some(p) = up {
            if chain.len() >= 4 {
                break;
            }
            let classes = p.css_classes().iter().map(|c| c.to_string()).collect::<Vec<_>>();
            chain.push(format!("{}.{}", p.type_().name(), classes.join(".")));
            up = p.parent();
        }
        text.push(format!("in:{}", chain.join("<")));
    }
    let joined: String = text.join(" / ").chars().take(160).collect();
    format!("{out} [{joined}] mapped={}", widget.is_mapped())
}

fn focus_line(shell: &Rc<Shell>) -> String {
    let window: gtk::Window = shell.ctx.window.clone().upcast();
    match gtk::prelude::GtkWindowExt::focus(&window) {
        Some(w) => describe(&w),
        None => "none".into(),
    }
}

thread_local! {
    static GAP: std::cell::Cell<(Option<std::time::Instant>, u128)> =
        const { std::cell::Cell::new((None, 0)) };
}

/// The longest the main loop went without running a 10 ms tick, since the last probe.
fn watch_stalls() {
    glib::timeout_add_local(Duration::from_millis(10), || {
        let now = std::time::Instant::now();
        GAP.with(|g| {
            let (last, max) = g.get();
            let gap = last.map_or(0, |l| now.duration_since(l).as_millis());
            g.set((Some(now), max.max(gap)));
        });
        glib::ControlFlow::Continue
    });
}

thread_local! {
    static FRAMES: std::cell::RefCell<Option<(glib::SignalHandlerId, Vec<std::time::Instant>)>> =
        const { std::cell::RefCell::new(None) };
}

/// `frames start` notes when each frame is painted; `frames stop` logs the gaps between them
/// (median, 95th percentile, longest, how many over 50 ms) and the widgets and pictures mapped.
fn frames(shell: &Rc<Shell>, what: &str) {
    let window = shell.ctx.window.clone();
    let Some(clock) = window.frame_clock() else {
        tracing::warn!("HUNT frames: no frame clock");
        return;
    };
    if what == "start" {
        take_stall();
        let id = clock.connect_after_paint(|_| {
            FRAMES.with(|f| {
                if let Some((_, times)) = f.borrow_mut().as_mut() {
                    times.push(std::time::Instant::now());
                }
            })
        });
        FRAMES.with(|f| *f.borrow_mut() = Some((id, Vec::new())));
        tracing::info!("HUNT frames start");
        return;
    }
    let Some((id, times)) = FRAMES.with(|f| f.borrow_mut().take()) else { return };
    clock.disconnect(id);
    let mut gaps: Vec<u128> = times.windows(2).map(|w| (w[1] - w[0]).as_millis()).collect();
    gaps.sort_unstable();
    let at = |q: f64| gaps.get(((gaps.len() as f64 - 1.0) * q).round() as usize).copied();
    let (mut widgets, mut pictures) = (0, 0);
    fn walk(w: &gtk::Widget, widgets: &mut u32, pictures: &mut u32) {
        if !w.is_mapped() {
            return;
        }
        *widgets += 1;
        if w.is::<gtk::Picture>() {
            *pictures += 1;
        }
        let mut c = w.first_child();
        while let Some(n) = c {
            walk(&n, widgets, pictures);
            c = n.next_sibling();
        }
    }
    walk(window.upcast_ref(), &mut widgets, &mut pictures);
    tracing::info!(
        "HUNT frames {what}: frames={} p50_ms={:?} p95_ms={:?} max_ms={:?} over50={} \
         max_stall_ms={} mapped_widgets={widgets} mapped_pictures={pictures}",
        times.len(),
        at(0.5),
        at(0.95),
        gaps.last(),
        gaps.iter().filter(|g| **g > 50).count(),
        take_stall(),
    );
}

fn take_stall() -> u128 {
    GAP.with(|g| {
        let (last, max) = g.get();
        g.set((last, 0));
        max
    })
}

pub fn probe(shell: &Rc<Shell>, tag: &str) {
    tracing::info!(
        "HUNT {tag}: {} max_stall_ms={} | top={:?} | {} | focus={}",
        proc_facts(),
        take_stall(),
        shell.top.visible_child_name(),
        shell.reader.hunt_facts(),
        focus_line(shell)
    );
}

/// Log every move of the keyboard focus, for the keyboard drives.
fn follow_focus(shell: &Rc<Shell>) {
    let window: gtk::Window = shell.ctx.window.clone().upcast();
    window.connect_notify_local(Some("focus-widget"), |window, _| {
        let what = gtk::prelude::GtkWindowExt::focus(window)
            .map(|w| describe(&w))
            .unwrap_or_else(|| "none".into());
        tracing::info!("HUNT focus-> {what}");
    });
    // Every key the window hears, before anything inside it, for the keyboard drives.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|_, key, code, modifiers| {
        tracing::info!("HUNT keyin {:?} code={code} mods={modifiers:?}", key.name());
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
}

/// How wide the window's content needs to be, against how wide it is.
fn width_need(shell: &Rc<Shell>, tag: &str) {
    let window = shell.ctx.window.clone();
    let have = window.width();
    let (min, nat, _, _) = window.measure(gtk::Orientation::Horizontal, -1);
    let content = window.content().map(|c| c.measure(gtk::Orientation::Horizontal, -1).0);
    // The widest mapped descendant past the window's right edge.
    let mut over: Vec<(i32, String)> = Vec::new();
    fn walk(w: &gtk::Widget, root: &gtk::Widget, have: i32, over: &mut Vec<(i32, String)>) {
        if !w.is_mapped() {
            return;
        }
        if let Some(b) = w.compute_bounds(root) {
            let right = (b.x() + b.width()) as i32;
            if right > have + 1 && (w.is::<gtk::Label>() || w.is::<gtk::Button>()) {
                over.push((right, describe(w)));
            }
        }
        let mut c = w.first_child();
        while let Some(n) = c {
            walk(&n, root, have, over);
            c = n.next_sibling();
        }
    }
    walk(window.upcast_ref(), window.upcast_ref(), have, &mut over);
    over.sort_by(|a, b| b.0.cmp(&a.0));
    over.dedup_by(|a, b| a.1 == b.1);
    let list: Vec<String> = over.iter().take(6).map(|(r, d)| format!("{r}:{d}")).collect();
    tracing::info!(
        "HUNT width {tag}: have={have} window_min={min} window_nat={nat} content_min={content:?} \
         past_edge={} {}",
        over.len(),
        list.join(" || ")
    );
}

async fn wait_ms(ms: u64) {
    glib::timeout_future(Duration::from_millis(ms)).await;
}

fn read(shell: &Rc<Shell>, work: &str, chapter: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.read",
        Some(&(work, chapter).to_variant()),
    );
}

fn series(shell: &Rc<Shell>, work: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-series",
        Some(&work.to_variant()),
    );
}

async fn shot(shell: &Rc<Shell>, name: &str) {
    let run = std::env::var("NAME").unwrap_or_default();
    let path = std::path::PathBuf::from(format!("{}/{run}-{name}.png", out_dir()));
    let _ = std::fs::create_dir_all(out_dir());
    match crate::shots::painted(shell, &path, name).await {
        Ok(()) => tracing::info!("HUNT shot {}", path.display()),
        Err(e) => tracing::warn!("HUNT shot {name} failed: {e}"),
    }
}

fn find<T: IsA<gtk::Widget>>(widget: &gtk::Widget, test: &dyn Fn(&T) -> bool) -> Option<T> {
    if widget.is_mapped()
        && let Some(w) = widget.downcast_ref::<T>()
        && test(w)
    {
        return Some(w.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find(&next, test) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

fn find_entry(widget: &gtk::Widget) -> Option<gtk::SearchEntry> {
    find::<gtk::SearchEntry>(widget, &|_| true)
}

/// Every label's text on screen under `widget`, for a log line.
pub fn labels(widget: &gtk::Widget, out: &mut Vec<String>) {
    if !widget.is_mapped() {
        return;
    }
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        let text = label.text().to_string();
        if !text.trim().is_empty() {
            out.push(text);
        }
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        labels(&next, out);
        child = next.next_sibling();
    }
}

fn screen_text(shell: &Rc<Shell>) -> String {
    let mut out = Vec::new();
    labels(shell.ctx.window.upcast_ref(), &mut out);
    let n = out.len();
    let joined = out.join(" | ");
    let cut: String = joined.chars().take(1500).collect();
    format!("({n} labels) {cut}")
}

fn send(shell: &Rc<Shell>, c: kaeri_model::reading::ReadingCommand) {
    shell.ctx.hub.send(kaeri_model::command::Command::Reading(c));
}

fn net(shell: &Rc<Shell>, offline: bool) {
    shell.ctx.hub.send(kaeri_model::command::Command::NetworkPath {
        cellular: false,
        constrained: false,
        offline,
    });
}

/// The book downloads folder, listed into the log.
fn files_line() {
    if let Ok(out) = std::process::Command::new("sh")
        .arg("-c")
        .arg("cd \"$HUNT_DATA/data\" 2>/dev/null && df -k . | tail -1; ls -R book-downloads 2>/dev/null | head -60; ls streamed 2>/dev/null | head; ls -R ../cache 2>/dev/null | grep -c png")
        .output()
    {
        tracing::info!("HUNT files: {}", String::from_utf8_lossy(&out.stdout).replace('\n', " / "));
    }
}

// ---------------------------------------------------------------------------------------------
// Drives

/// The stage for `shot`, when it is a hunt drive: how long the run should wait for it.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let secs = match shot {
        "hunt-cycle-manga" | "hunt-cycle-strip" | "hunt-cycle-manga-noturn" => 180.0,
        "hunt-cycle-novel" => 300.0,
        "hunt-midload0" => 60.0,
        "hunt-keyspam" | "hunt-keyspam-strip" | "hunt-keyspam-novel" => 40.0,
        "hunt-offline-flip" => 60.0,
        "hunt-narrow" => 150.0,
        "hunt-x-novel" | "hunt-x-manga" | "hunt-x-strip" => 120.0,
        "hunt-x-series" | "hunt-x-home" | "hunt-x-settings" => 150.0,
        "hunt-tide-series" => 160.0,
        "hunt-tide-pages" => 120.0,
        "hunt-tide-150" => 60.0,
        "hunt-cross30" => 150.0,
        "hunt-tall" => 80.0,
        "hunt-ruby" => 70.0,
        "hunt-oneshot" => 30.0,
        "hunt-disk" => 60.0,
        "hunt-dl-kill" => 600.0,
        "hunt-after-kill" => 150.0,
        "hunt-delete-reading" => 40.0,
        "hunt-sleep" => 120.0,
        "hunt-real" => 420.0,
        "hunt-big-nopage" => 150.0,
        "hunt-dl-read" => 200.0,
        "hunt-dl-read-kill" => 120.0,
        "hunt-disk2" => 150.0,
        "hunt-real2" => 1500.0,
        "hunt-x-kb" => 300.0,
        "hunt-x-kb-reader" => 300.0,
        "hunt-mode" => 400.0,
        "hunt-a1" => 120.0,
        "hunt-a2" => 150.0,
        "hunt-a3" => 120.0,
        "hunt-close" => 120.0,
        "hunt-rsearch" => 200.0,
        "hunt-p11" => 600.0,
        "hunt-do" => std::env::var("HUNT_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(600.0),
        _ => return None,
    };
    let shell = Rc::clone(shell);
    let name = shot.to_owned();
    follow_focus(&shell);
    watch_stalls();
    if let Some(dpi) = std::env::var("HUNT_DPI").ok().and_then(|d| d.parse::<i32>().ok())
        && let Some(settings) = gtk::Settings::default()
    {
        settings.set_gtk_xft_dpi(dpi);
        tracing::info!("HUNT dpi set to {dpi}");
    }
    glib::spawn_future_local(async move { drive(&shell, &name).await });
    Some(secs)
}

/// Shots the xdotool script asks for, by a file `/out/hunt/req-<tag>`: an in-app shot and a
/// probe, since the X root shows nothing of a GL window.
fn serve_requests(shell: &Rc<Shell>) {
    let shell = Rc::clone(shell);
    glib::spawn_future_local(async move {
        loop {
            wait_ms(300).await;
            let Ok(dir) = std::fs::read_dir(out_dir()) else { continue };
            let mut asks: Vec<String> = dir
                .flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|n| n.starts_with("req-"))
                .collect();
            asks.sort();
            for ask in asks {
                let _ = std::fs::remove_file(format!("{}/{ask}", out_dir()));
                let tag = ask.trim_start_matches("req-").to_owned();
                probe(&shell, &format!("x {tag}"));
                tracing::info!("HUNT x {tag} text: {}", screen_text(&shell));
                shot(&shell, &format!("x-{tag}")).await;
            }
        }
    });
}

async fn drive(shell: &Rc<Shell>, name: &str) {
    // Fixtures never say the network, which holds the book download queue: say online.
    if std::env::var("HUNT_NONET").is_err() {
        net(shell, false);
    }
    if name.starts_with("hunt-x-") || std::env::var("HUNT_XDO").is_ok() {
        serve_requests(shell);
    }
    match name {
        "hunt-do" => do_steps(shell).await,
        "hunt-cycle-manga" | "hunt-cycle-strip" | "hunt-cycle-manga-noturn" => {
            let (work, chapter) =
                if name.contains("manga") { (MANGA, "13") } else { (STRIP, "2") };
            probe(shell, &format!("{name} start"));
            for i in 1..=50 {
                read(shell, work, chapter);
                wait_ms(1200).await;
                if !name.ends_with("noturn") {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                }
                wait_ms(150).await;
                shell.reader.close();
                wait_ms(400).await;
                if i % 5 == 0 || i <= 2 {
                    probe(shell, &format!("{name} {i}"));
                }
            }
            wait_ms(5000).await;
            probe(shell, &format!("{name} idle 5s after close"));
            unsafe { libc::malloc_trim(0) };
            probe(shell, &format!("{name} after malloc_trim"));
        }
        "hunt-cycle-novel" => {
            probe(shell, "novel start");
            for i in 1..=50 {
                read(shell, NOVEL, "3");
                wait_ms(2500).await;
                if i == 1 {
                    shot(shell, "novel-first-open").await;
                }
                shell.reader.pad(ReaderPad::Turn(Direction::Right));
                wait_ms(300).await;
                shell.reader.close();
                wait_ms(800).await;
                if i % 5 == 0 || i <= 3 {
                    probe(shell, &format!("novel {i}"));
                }
            }
            wait_ms(8000).await;
            probe(shell, "novel idle 8s after close");
            unsafe { libc::malloc_trim(0) };
            probe(shell, "novel after malloc_trim");
        }
        "hunt-midload0" => {
            probe(shell, "midload0 start");
            for (work, chapter) in [(NOVEL, "3"), (MANGA, "13"), (STRIP, "2"), (BIG, "3")] {
                read(shell, work, chapter);
                shell.reader.close();
                probe(shell, &format!("midload0 {work} closed at once"));
                wait_ms(5000).await;
                probe(shell, &format!("midload0 {work} 5s after close"));
            }
            // Leaving with Esc a few frames in, then opening another work at once.
            for (work, chapter) in [(NOVEL, "3"), (MANGA, "13")] {
                read(shell, work, chapter);
                wait_ms(16).await;
                shell.reader.hunt_key(gtk::gdk::Key::Escape);
                read(shell, STRIP, "1");
                wait_ms(4000).await;
                probe(shell, &format!("midload0 esc {work} then strip 1"));
                shell.reader.close();
                wait_ms(3000).await;
                probe(shell, &format!("midload0 esc {work} closed"));
            }
            shot(shell, "midload0-after").await;
        }
        "hunt-keyspam" | "hunt-keyspam-strip" | "hunt-keyspam-novel" => {
            let (work, chapter) = match name {
                "hunt-keyspam" => (MANGA, "13"),
                "hunt-keyspam-strip" => (STRIP, "1"),
                _ => (NOVEL, "2"),
            };
            read(shell, work, chapter);
            wait_ms(3000).await;
            probe(shell, &format!("{name} opened"));
            for i in 0..400 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                if i % 4 == 0 {
                    wait_ms(1).await;
                }
            }
            wait_ms(4000).await;
            probe(shell, &format!("{name} after 400 forward"));
            shot(shell, &format!("{name}-forward")).await;
            for i in 0..400 {
                shell.reader.pad(ReaderPad::Turn(Direction::Up));
                if i % 4 == 0 {
                    wait_ms(1).await;
                }
            }
            wait_ms(4000).await;
            probe(shell, &format!("{name} after 400 back"));
            shot(shell, &format!("{name}-back")).await;
            for key in [gtk::gdk::Key::End, gtk::gdk::Key::Home] {
                shell.reader.hunt_key(key);
                wait_ms(2000).await;
                probe(shell, &format!("{name} after {}", key.name().unwrap_or_default()));
                shot(shell, &format!("{name}-{}", key.name().unwrap_or_default())).await;
            }
            for i in 0..60 {
                shell.reader.pad(ReaderPad::Chapter(if i % 2 == 0 { 1 } else { -1 }));
                wait_ms(5).await;
            }
            wait_ms(4000).await;
            probe(shell, &format!("{name} after chapter spam"));
            shot(shell, &format!("{name}-chapters")).await;
            shell.reader.close();
        }
        "hunt-offline-flip" => {
            for (dest, tag) in
                [(Dest::ReadLibrary, "library"), (Dest::ReadDownloads, "downloads")]
            {
                shell.go(dest);
                wait_ms(2500).await;
                tracing::info!("HUNT before flips {tag}: {}", screen_text(shell));
                for i in 0..30 {
                    net(shell, i % 2 == 0);
                    wait_ms(50).await;
                }
                net(shell, true);
                wait_ms(3000).await;
                tracing::info!("HUNT offline after flips {tag}: {}", screen_text(shell));
                shot(shell, &format!("flips-{tag}-offline")).await;
                net(shell, false);
                wait_ms(3000).await;
                tracing::info!("HUNT online after flips {tag}: {}", screen_text(shell));
                shot(shell, &format!("flips-{tag}")).await;
            }
            // Offline: open the downloaded chapter, cross on, and flip mid chapter.
            net(shell, true);
            wait_ms(500).await;
            read(shell, MANGA, "14");
            wait_ms(3000).await;
            probe(shell, "offline reader 14 opened");
            for _ in 0..24 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(80).await;
            }
            wait_ms(3000).await;
            probe(shell, "offline reader crossed from 14");
            tracing::info!("HUNT offline reader text: {}", screen_text(shell));
            shot(shell, "offline-reader").await;
            for i in 0..20 {
                net(shell, i % 2 == 1);
                shell.reader.pad(ReaderPad::Turn(Direction::Up));
                wait_ms(100).await;
            }
            net(shell, false);
            wait_ms(3000).await;
            probe(shell, "online reader after flips");
            shot(shell, "online-reader").await;
            shell.reader.close();
            wait_ms(1500).await;
            shell.go(Dest::ReadDownloads);
            wait_ms(2000).await;
            tracing::info!("HUNT downloads after reading: {}", screen_text(shell));
            shot(shell, "downloads-after-reading").await;
        }
        "hunt-narrow" => {
            let places: Vec<(&str, Box<dyn Fn(&Rc<Shell>)>)> = vec![
                ("read-home", Box::new(|s| s.go(Dest::Read))),
                ("read-search", Box::new(|s| s.go(Dest::ReadSearch))),
                ("read-library", Box::new(|s| s.go(Dest::ReadLibrary))),
                ("read-downloads", Box::new(|s| s.go(Dest::ReadDownloads))),
                ("read-sources", Box::new(|s| s.go(Dest::ReadSources))),
                ("series-manga", Box::new(|s| series(s, MANGA))),
                ("series-novel", Box::new(|s| series(s, NOVEL))),
                ("series-strip", Box::new(|s| series(s, STRIP))),
                ("series-big", Box::new(|s| series(s, BIG))),
                ("reader-manga", Box::new(|s| read(s, MANGA, "13"))),
                ("reader-novel", Box::new(|s| read(s, NOVEL, "3"))),
                ("reader-strip", Box::new(|s| read(s, STRIP, "2"))),
            ];
            for (tag, go) in places {
                go(shell);
                wait_ms(3500).await;
                width_need(shell, tag);
                shot(shell, &format!("narrow-{tag}")).await;
                if tag.starts_with("reader") {
                    shell.reader.pad(ReaderPad::Settings);
                    wait_ms(1200).await;
                    width_need(shell, &format!("{tag}-settings"));
                    shot(shell, &format!("narrow-{tag}-settings")).await;
                    shell.reader.hunt_key(gtk::gdk::Key::Escape);
                    wait_ms(500).await;
                    shell.reader.shot_end();
                    wait_ms(1500).await;
                    width_need(shell, &format!("{tag}-end"));
                    shot(shell, &format!("narrow-{tag}-end")).await;
                    shell.reader.close();
                    wait_ms(1500).await;
                }
            }
        }
        "hunt-x-novel" | "hunt-x-manga" | "hunt-x-strip" => {
            // Held open for an outside driver (xdotool) after a reader opens.
            let (work, chapter) = match name {
                "hunt-x-novel" => (NOVEL, "2"),
                "hunt-x-manga" => (MANGA, "13"),
                _ => (STRIP, "1"),
            };
            read(shell, work, chapter);
            for i in 0..22 {
                wait_ms(5000).await;
                probe(shell, &format!("{name} {i}"));
            }
        }
        "hunt-x-series" => {
            series(shell, MANGA);
            wait_ms(3000).await;
            // The series' main action takes the keyboard, as Tab would put it there.
            if let Some(button) = find::<gtk::Button>(shell.ctx.window.upcast_ref(), &|b| {
                b.action_name().as_deref() == Some("win.read")
                    && b.has_css_class("suggested-action")
            })
            .or_else(|| {
                find::<gtk::Button>(shell.ctx.window.upcast_ref(), &|b| {
                    b.action_name().as_deref() == Some("win.read")
                })
            }) {
                button.grab_focus();
            }
            probe(shell, "x-series ready");
            for i in 0..28 {
                wait_ms(5000).await;
                probe(shell, &format!("x-series {i}"));
            }
        }
        "hunt-x-home" => {
            shell.go(Dest::Read);
            for i in 0..28 {
                wait_ms(5000).await;
                probe(shell, &format!("x-home {i}"));
            }
        }
        "hunt-x-settings" => {
            let _ = WidgetExt::activate_action(
                &shell.ctx.window,
                "win.settings-pane",
                Some(&"advanced".to_variant()),
            );
            wait_ms(2500).await;
            if let Some(row) = find::<adw::EntryRow>(shell.ctx.window.upcast_ref(), &|_| true) {
                tracing::info!("HUNT mpv row text={:?}", row.text());
                row.grab_focus();
            }
            for i in 0..28 {
                wait_ms(4000).await;
                let config = std::fs::read_to_string(
                    std::path::Path::new(&std::env::var("HUNT_DATA").unwrap_or_default())
                        .join("config/config.toml"),
                )
                .unwrap_or_else(|e| format!("<{e}>"));
                let mpv: Vec<&str> =
                    config.lines().filter(|l| l.contains("mpv_args")).collect();
                let row = find::<adw::EntryRow>(shell.ctx.window.upcast_ref(), &|_| true)
                    .map(|r| r.text().to_string());
                tracing::info!(
                    "HUNT x-settings {i}: config mpv={mpv:?} row={row:?} text={}",
                    screen_text(shell)
                );
            }
        }
        "hunt-tide-series" => {
            let t0 = std::time::Instant::now();
            series(shell, BIG);
            for i in 0..6 {
                wait_ms(1000).await;
                tracing::info!(
                    "HUNT tide series at {}ms: {}",
                    t0.elapsed().as_millis(),
                    screen_text(shell)
                );
                if i == 5 {
                    shot(shell, "tide-series").await;
                }
            }
            probe(shell, "tide series shown");
            shell.go(Dest::Read);
            wait_ms(2000).await;
            series(shell, BIG);
            wait_ms(6000).await;
            tracing::info!("HUNT tide series second visit: {}", screen_text(shell));
            shot(shell, "tide-series-again").await;
            series(shell, TALL);
            wait_ms(6000).await;
            tracing::info!("HUNT tall series: {}", screen_text(shell));
            shot(shell, "tall-series").await;
            series(shell, "a:900309");
            wait_ms(6000).await;
            tracing::info!("HUNT oneshot series: {}", screen_text(shell));
            shot(shell, "oneshot-series").await;
            // Reading from the long series and back, ten times: its page redraws on each close.
            series(shell, BIG);
            wait_ms(6000).await;
            probe(shell, "big series before cycles");
            for i in 1..=10 {
                read(shell, BIG, "5");
                wait_ms(3000).await;
                probe(shell, &format!("big cycle {i} open"));
                shell.reader.close();
                wait_ms(3000).await;
                probe(shell, &format!("big cycle {i} closed"));
            }
            shell.reader.hunt_key(gtk::gdk::Key::Escape);
            read(shell, BIG, "5");
            wait_ms(3000).await;
            shell.reader.pad(ReaderPad::Settings);
            wait_ms(500).await;
            probe(shell, "big settings open");
            shell.reader.hunt_key(gtk::gdk::Key::Escape);
            wait_ms(500).await;
            let t0 = std::time::Instant::now();
            shell.reader.hunt_chapters();
            tracing::info!("HUNT chapters popup call took {}ms", t0.elapsed().as_millis());
            wait_ms(2500).await;
            probe(shell, "big chapters popover (c)");
            shot(shell, "big-chapters-popover").await;
            shell.reader.close();
        }
        "hunt-tide-pages" => {
            read(shell, BIG, "2");
            wait_ms(4000).await;
            probe(shell, "tide-pages opened");
            shot(shell, "tide-pages-0").await;
            for i in 1..=19 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(2500).await;
                probe(shell, &format!("tide-pages turn {i}"));
                tracing::info!("HUNT tide page {i} text: {}", screen_text(shell));
                if matches!(i, 2 | 4 | 6 | 8 | 10 | 12 | 14 | 16) {
                    shot(shell, &format!("tide-pages-{i}")).await;
                }
            }
            for i in 0..19 {
                shell.reader.pad(ReaderPad::Turn(Direction::Up));
                wait_ms(150).await;
                let _ = i;
            }
            wait_ms(3000).await;
            probe(shell, "tide-pages back at start");
            shell.reader.close();
            wait_ms(2000).await;
            probe(shell, "tide-pages closed");
        }
        "hunt-tide-150" => {
            read(shell, BIG, "1");
            wait_ms(4000).await;
            probe(shell, "tide-150 opened");
            shell.reader.hunt_key(gtk::gdk::Key::End);
            wait_ms(3000).await;
            probe(shell, "tide-150 End");
            shot(shell, "tide-150-end").await;
            for _ in 0..150 {
                shell.reader.pad(ReaderPad::Turn(Direction::Up));
                wait_ms(20).await;
            }
            wait_ms(4000).await;
            probe(shell, "tide-150 back 150");
            shot(shell, "tide-150-back").await;
            shell.reader.close();
            wait_ms(2000).await;
            probe(shell, "tide-150 closed");
        }
        "hunt-cross30" => {
            read(shell, BIG, "3");
            wait_ms(4000).await;
            probe(shell, "cross30 start");
            for i in 1..=30 {
                shell.reader.pad(ReaderPad::Chapter(1));
                wait_ms(1500).await;
                if i % 5 == 0 {
                    probe(shell, &format!("cross30 forward {i}"));
                }
            }
            shot(shell, "cross30-forward").await;
            for i in 1..=30 {
                shell.reader.pad(ReaderPad::Chapter(-1));
                wait_ms(120).await;
                let _ = i;
            }
            wait_ms(5000).await;
            probe(shell, "cross30 back fast");
            shot(shell, "cross30-back").await;
            // Crossing by page turns: through the end page into the next chapter, ten times.
            for i in 1..=10 {
                for _ in 0..14 {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                    wait_ms(60).await;
                }
                wait_ms(1500).await;
                probe(shell, &format!("cross30 by turns {i}"));
            }
            shot(shell, "cross30-turns").await;
            shell.reader.close();
            wait_ms(3000).await;
            probe(shell, "cross30 closed");
        }
        "hunt-tall" => {
            read(shell, TALL, "1");
            wait_ms(5000).await;
            probe(shell, "tall opened");
            for i in 0..120 {
                shell.reader.hunt_key(gtk::gdk::Key::Page_Down);
                wait_ms(60).await;
                if i % 30 == 29 {
                    probe(shell, &format!("tall pagedown {i}"));
                }
            }
            wait_ms(3000).await;
            shot(shell, "tall-after-pagedown").await;
            shell.reader.hunt_key(gtk::gdk::Key::Home);
            wait_ms(3000).await;
            probe(shell, "tall Home");
            shot(shell, "tall-home").await;
            shell.reader.hunt_key(gtk::gdk::Key::End);
            wait_ms(4000).await;
            probe(shell, "tall End");
            shot(shell, "tall-end").await;
            wait_ms(15000).await;
            probe(shell, "tall End 15s");
            shot(shell, "tall-end-15s").await;
            shell.reader.close();
            wait_ms(2000).await;
            probe(shell, "tall closed");
        }
        "hunt-ruby" => {
            read(shell, RUBY, "1");
            wait_ms(5000).await;
            probe(shell, "ruby opened");
            shot(shell, "ruby").await;
            shell.reader.hunt_key(gtk::gdk::Key::End);
            wait_ms(1500).await;
            probe(shell, "ruby End");
            shot(shell, "ruby-end-key").await;
            shell.reader.hunt_key(gtk::gdk::Key::Home);
            wait_ms(1500).await;
            probe(shell, "ruby Home");
            shell.reader.pad(ReaderPad::Turn(Direction::Down));
            wait_ms(1500).await;
            probe(shell, "ruby page 2");
            shot(shell, "ruby-page2").await;
            shell.reader.pad(ReaderPad::Turn(Direction::Down));
            wait_ms(1500).await;
            probe(shell, "ruby page 3");
            shot(shell, "ruby-page3").await;
            wait_ms(30000).await;
            probe(shell, "ruby page 3 after 30s");
            shot(shell, "ruby-page3-30s").await;
            shell.reader.close();
        }
        "hunt-oneshot" => {
            series(shell, "a:900309");
            wait_ms(6000).await;
            tracing::info!("HUNT oneshot: {}", screen_text(shell));
            shot(shell, "oneshot").await;
        }
        "hunt-disk" => {
            series(shell, BIG);
            wait_ms(4000).await;
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(BIG),
                    pick: kaeri_model::reading::DownloadPick::NextUnread { n: 3 },
                },
            );
            wait_ms(6000).await;
            tracing::info!("HUNT disk series: {}", screen_text(shell));
            shot(shell, "disk-series").await;
            shell.go(Dest::ReadDownloads);
            for i in 0..6 {
                wait_ms(4000).await;
                tracing::info!("HUNT disk downloads {i}: {}", screen_text(shell));
            }
            shot(shell, "disk-downloads").await;
            if let Ok(out) = std::process::Command::new("sh")
                .arg("-c")
                .arg("df -k $HUNT_DATA/data/book-downloads; ls -laR $HUNT_DATA/data/book-downloads | head -30")
                .output()
            {
                tracing::info!("HUNT disk files: {}", String::from_utf8_lossy(&out.stdout).replace('\n', " / "));
            }
        }
        "hunt-dl-kill" => {
            series(shell, BIG);
            wait_ms(4000).await;
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(BIG),
                    pick: kaeri_model::reading::DownloadPick::NextUnread { n: 4 },
                },
            );
            shell.go(Dest::ReadDownloads);
            for i in 0..40 {
                wait_ms(1000).await;
                if i % 5 == 0 {
                    tracing::info!("HUNT dl-kill {i}s: {}", screen_text(shell));
                }
            }
            shot(shell, "dl-kill-before").await;
            tracing::info!("HUNT dl-kill aborting now");
            std::process::abort();
        }
        "hunt-after-kill" => {
            shell.go(Dest::ReadDownloads);
            for i in 0..8 {
                wait_ms(3000).await;
                tracing::info!("HUNT after-kill {}s: {}", (i + 1) * 3, screen_text(shell));
            }
            shot(shell, "after-kill").await;
            let ch = std::env::var("HUNT_AFTER_CH").unwrap_or_else(|_| "1".into());
            files_line();
            read(shell, BIG, &ch);
            wait_ms(4000).await;
            probe(shell, &format!("after-kill reader {ch}"));
            for _ in 0..13 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(800).await;
            }
            wait_ms(2000).await;
            probe(shell, &format!("after-kill reader {ch} through"));
            tracing::info!("HUNT after-kill reader text: {}", screen_text(shell));
            shell.reader.close();
            wait_ms(20000).await;
            shell.go(Dest::ReadDownloads);
            wait_ms(3000).await;
            tracing::info!("HUNT after-kill downloads at the end: {}", screen_text(shell));
            files_line();
            shot(shell, "after-kill-reader").await;
        }
        "hunt-delete-reading" => {
            read(shell, MANGA, "14");
            wait_ms(3000).await;
            probe(shell, "delete-reading opened 14");
            for _ in 0..3 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(200).await;
            }
            send(
                shell,
                kaeri_model::reading::ReadingCommand::DownloadDelete {
                    work: kaeri_core::books::WorkId::new(MANGA),
                    chapters: vec![kaeri_core::books::ChapterId::new("14")],
                },
            );
            wait_ms(1500).await;
            probe(shell, "delete-reading deleted");
            for i in 0..24 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(250).await;
                if i % 6 == 5 {
                    probe(shell, &format!("delete-reading turn {i}"));
                }
            }
            tracing::info!("HUNT delete-reading text: {}", screen_text(shell));
            shot(shell, "delete-reading").await;
            shell.reader.close();
            wait_ms(1500).await;
            read(shell, MANGA, "14");
            wait_ms(4000).await;
            probe(shell, "delete-reading reopened 14");
            tracing::info!("HUNT delete-reading reopen text: {}", screen_text(shell));
            shot(shell, "delete-reading-reopen").await;
            shell.reader.close();
        }
        "hunt-real" => {
            // Real sources, gently: one step every few seconds, a handful of pages.
            let steps: [(&str, &str, &str, u32); 3] = [
                ("a:105398", "1", "strip-4-sources", 6),
                ("a:97852", "1", "manga-537", 6),
                ("a:85737", "1", "novel", 4),
            ];
            for (work, chapter, tag, turns) in steps {
                let t0 = std::time::Instant::now();
                series(shell, work);
                for i in 0..4 {
                    wait_ms(3000).await;
                    tracing::info!(
                        "HUNT real {tag} series at {}ms: {}",
                        t0.elapsed().as_millis(),
                        screen_text(shell)
                    );
                    if i == 3 {
                        shot(shell, &format!("real-{tag}-series")).await;
                    }
                }
                let t0 = std::time::Instant::now();
                read(shell, work, chapter);
                for i in 0..10 {
                    wait_ms(1500).await;
                    let facts = shell.reader.hunt_facts();
                    tracing::info!(
                        "HUNT real {tag} open at {}ms: {facts}",
                        t0.elapsed().as_millis()
                    );
                    if !facts.contains("waiting=Some(Chapter)") && i >= 3 {
                        break;
                    }
                }
                wait_ms(3000).await;
                probe(shell, &format!("real {tag} opened"));
                shot(shell, &format!("real-{tag}-reader")).await;
                for i in 0..turns {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                    wait_ms(2500).await;
                    if i == turns - 1 {
                        probe(shell, &format!("real {tag} after {turns} turns"));
                        shot(shell, &format!("real-{tag}-turned")).await;
                    }
                }
                net(shell, true);
                for _ in 0..3 {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                    wait_ms(2000).await;
                }
                probe(shell, &format!("real {tag} offline"));
                tracing::info!("HUNT real {tag} offline text: {}", screen_text(shell));
                shot(shell, &format!("real-{tag}-offline")).await;
                net(shell, false);
                wait_ms(4000).await;
                probe(shell, &format!("real {tag} back online"));
                shell.reader.close();
                wait_ms(3000).await;
                tracing::info!("HUNT real {tag} series after close: {}", screen_text(shell));
                shot(shell, &format!("real-{tag}-after")).await;
            }
            probe(shell, "real done");
        }
        "hunt-big-nopage" => {
            shell.go(Dest::Read);
            wait_ms(4000).await;
            probe(shell, "nopage start");
            for i in 1..=3 {
                let t0 = std::time::Instant::now();
                read(shell, BIG, "5");
                wait_ms(6000).await;
                probe(shell, &format!("nopage {i} open after {}ms", t0.elapsed().as_millis()));
                let t0 = std::time::Instant::now();
                shell.reader.close();
                wait_ms(6000).await;
                probe(
                    shell,
                    &format!("nopage {i} closed after {}ms", t0.elapsed().as_millis()),
                );
            }
            read(shell, BIG, "5");
            wait_ms(8000).await;
            probe(shell, "nopage before chapters");
            let t0 = std::time::Instant::now();
            shell.reader.hunt_chapters();
            wait_ms(100).await;
            tracing::info!("HUNT chapters popup took {}ms", t0.elapsed().as_millis());
            wait_ms(5000).await;
            probe(shell, "nopage chapters open");
            shot(shell, "nopage-chapters").await;
            shell.reader.close();
            wait_ms(3000).await;
            series(shell, BIG);
            wait_ms(15000).await;
            probe(shell, "nopage series shown");
            for i in 1..=2 {
                let t0 = std::time::Instant::now();
                read(shell, BIG, "5");
                wait_ms(100).await;
                tracing::info!(
                    "HUNT series open {i} first tick after {}ms",
                    t0.elapsed().as_millis()
                );
                wait_ms(8000).await;
                let t0 = std::time::Instant::now();
                shell.reader.close();
                wait_ms(100).await;
                tracing::info!(
                    "HUNT series close {i} first tick after {}ms",
                    t0.elapsed().as_millis()
                );
                wait_ms(8000).await;
                probe(shell, &format!("series cycle {i} settled"));
            }
        }
        "hunt-dl-read" => {
            series(shell, BIG);
            wait_ms(3000).await;
            let big = kaeri_core::books::WorkId::new(BIG);
            let ids = |v: &[&str]| {
                v.iter().map(|c| kaeri_core::books::ChapterId::new(*c)).collect::<Vec<_>>()
            };
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: big.clone(),
                    pick: kaeri_model::reading::DownloadPick::Chapters {
                        ids: ids(&["30", "31"]),
                    },
                },
            );
            wait_ms(1500).await;
            read(shell, BIG, "30");
            for i in 0..4 {
                wait_ms(1500).await;
                probe(shell, &format!("dlread 30 while downloading t{i}"));
            }
            shot(shell, "dlread-30-open").await;
            for i in 1..=4 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(1500).await;
                probe(shell, &format!("dlread 30 turn {i}"));
            }
            send(
                shell,
                kaeri_model::reading::ReadingCommand::DownloadCancel {
                    work: big.clone(),
                    chapters: ids(&["30"]),
                },
            );
            wait_ms(500).await;
            probe(shell, "dlread 30 download cancelled");
            for i in 5..=14 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(1800).await;
                probe(shell, &format!("dlread 30 turn {i}"));
                tracing::info!("HUNT dlread 30 turn {i} text: {}", screen_text(shell));
            }
            shot(shell, "dlread-30-after-cancel").await;
            shell.reader.close();
            wait_ms(1500).await;
            shell.go(Dest::ReadDownloads);
            for i in 0..3 {
                wait_ms(3000).await;
                tracing::info!("HUNT dlread downloads {i}: {}", screen_text(shell));
            }
            shot(shell, "dlread-downloads").await;
            read(shell, BIG, "31");
            wait_ms(3000).await;
            probe(shell, "dlread 31 opened");
            for i in 1..=3 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(1200).await;
                let _ = i;
            }
            send(
                shell,
                kaeri_model::reading::ReadingCommand::DownloadDelete {
                    work: big.clone(),
                    chapters: ids(&["31"]),
                },
            );
            wait_ms(800).await;
            probe(shell, "dlread 31 deleted while reading");
            for i in 4..=14 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(1500).await;
                probe(shell, &format!("dlread 31 turn {i}"));
            }
            tracing::info!("HUNT dlread 31 end text: {}", screen_text(shell));
            shot(shell, "dlread-31-end").await;
            shell.reader.close();
            wait_ms(1500).await;
            // The chapter whose download was cancelled, again, and a fresh download opened at
            // once and closed at once.
            read(shell, BIG, "30");
            wait_ms(4000).await;
            probe(shell, "dlread 30 reopened");
            shell.reader.close();
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: big.clone(),
                    pick: kaeri_model::reading::DownloadPick::Chapters { ids: ids(&["32"]) },
                },
            );
            for i in 0..5 {
                read(shell, BIG, "32");
                wait_ms(300 + i * 200).await;
                shell.reader.close();
                wait_ms(200).await;
            }
            wait_ms(15000).await;
            shell.go(Dest::ReadDownloads);
            wait_ms(3000).await;
            tracing::info!("HUNT dlread downloads after 32 churn: {}", screen_text(shell));
            read(shell, BIG, "32");
            wait_ms(3000).await;
            probe(shell, "dlread 32 after churn");
            for i in 1..=13 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(400).await;
                let _ = i;
            }
            wait_ms(2000).await;
            probe(shell, "dlread 32 end");
            shell.reader.close();
            files_line();
        }
        "hunt-dl-read-kill" => {
            series(shell, BIG);
            wait_ms(3000).await;
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(BIG),
                    pick: kaeri_model::reading::DownloadPick::Chapters {
                        ids: vec![
                            kaeri_core::books::ChapterId::new("40"),
                            kaeri_core::books::ChapterId::new("41"),
                        ],
                    },
                },
            );
            wait_ms(1000).await;
            read(shell, BIG, "40");
            wait_ms(3000).await;
            shell.reader.pad(ReaderPad::Turn(Direction::Down));
            wait_ms(3000).await;
            probe(shell, "dlkill2 reading 40 while it downloads");
            files_line();
            tracing::info!("HUNT dlkill2 aborting now");
            std::process::abort();
        }
        "hunt-disk2" => {
            series(shell, BIG);
            wait_ms(3000).await;
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(BIG),
                    pick: kaeri_model::reading::DownloadPick::Chapters {
                        ids: ["50", "51", "52"]
                            .iter()
                            .map(|c| kaeri_core::books::ChapterId::new(*c))
                            .collect(),
                    },
                },
            );
            wait_ms(1000).await;
            read(shell, BIG, "50");
            for i in 1..=14 {
                wait_ms(1500).await;
                probe(shell, &format!("disk2 50 t{i}"));
                if i > 2 {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                }
            }
            tracing::info!("HUNT disk2 50 text: {}", screen_text(shell));
            shot(shell, "disk2-50").await;
            shell.reader.close();
            wait_ms(1500).await;
            shell.go(Dest::ReadDownloads);
            for i in 0..5 {
                wait_ms(3000).await;
                tracing::info!("HUNT disk2 downloads {i}: {}", screen_text(shell));
            }
            shot(shell, "disk2-downloads").await;
            files_line();
            read(shell, BIG, "51");
            wait_ms(5000).await;
            probe(shell, "disk2 51 opened");
            for _ in 0..13 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(700).await;
            }
            wait_ms(2000).await;
            probe(shell, "disk2 51 through");
            tracing::info!("HUNT disk2 51 text: {}", screen_text(shell));
            shell.reader.close();
            files_line();
        }
        "hunt-real2" => {
            // Real sources, paced by the engine's QA gate: HUNT_REAL="work:chapter:turns,...".
            let plan = std::env::var("HUNT_REAL").unwrap_or_default();
            shell.go(Dest::Read);
            for i in 0..6 {
                wait_ms(5000).await;
                tracing::info!("HUNT real home {i}: {}", screen_text(shell));
            }
            shot(shell, "real-home").await;
            for step in plan.split(',').filter(|s| !s.is_empty()) {
                let mut it = step.split(':');
                let work = format!("{}:{}", it.next().unwrap_or("a"), it.next().unwrap_or("0"));
                let chapter = it.next().unwrap_or("1").to_owned();
                let turns: u32 = it.next().and_then(|t| t.parse().ok()).unwrap_or(4);
                let tag = work.replace(':', "");
                let t0 = std::time::Instant::now();
                series(shell, &work);
                for i in 0..8 {
                    wait_ms(4000).await;
                    tracing::info!(
                        "HUNT real {tag} series at {}ms: {}",
                        t0.elapsed().as_millis(),
                        screen_text(shell)
                    );
                    if i == 7 {
                        shot(shell, &format!("real-{tag}-series")).await;
                    }
                }
                probe(shell, &format!("real {tag} series shown"));
                let t0 = std::time::Instant::now();
                read(shell, &work, &chapter);
                for i in 0..30 {
                    wait_ms(2000).await;
                    let facts = shell.reader.hunt_facts();
                    tracing::info!(
                        "HUNT real {tag} open at {}ms: {facts}",
                        t0.elapsed().as_millis()
                    );
                    if !facts.contains("waiting=Some(Chapter)") && i >= 4 {
                        break;
                    }
                }
                probe(shell, &format!("real {tag} opened"));
                shot(shell, &format!("real-{tag}-reader")).await;
                for i in 0..turns {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                    wait_ms(4000).await;
                    probe(shell, &format!("real {tag} turn {}", i + 1));
                }
                shot(shell, &format!("real-{tag}-turned")).await;
                let t0 = std::time::Instant::now();
                shell.reader.hunt_key(gtk::gdk::Key::End);
                for i in 0..12 {
                    wait_ms(5000).await;
                    probe(shell, &format!("real {tag} End +{}ms", t0.elapsed().as_millis()));
                    if i == 2 {
                        shot(shell, &format!("real-{tag}-end-key")).await;
                    }
                }
                shot(shell, &format!("real-{tag}-end-key-60s")).await;
                shell.reader.hunt_key(gtk::gdk::Key::Home);
                wait_ms(4000).await;
                probe(shell, &format!("real {tag} Home"));
                net(shell, true);
                for _ in 0..3 {
                    shell.reader.pad(ReaderPad::Turn(Direction::Down));
                    wait_ms(2000).await;
                }
                probe(shell, &format!("real {tag} offline"));
                tracing::info!("HUNT real {tag} offline text: {}", screen_text(shell));
                shot(shell, &format!("real-{tag}-offline")).await;
                net(shell, false);
                wait_ms(5000).await;
                probe(shell, &format!("real {tag} back online"));
                let t0 = std::time::Instant::now();
                shell.reader.close();
                wait_ms(100).await;
                tracing::info!(
                    "HUNT real {tag} close first tick after {}ms",
                    t0.elapsed().as_millis()
                );
                wait_ms(5000).await;
                probe(shell, &format!("real {tag} closed"));
                tracing::info!("HUNT real {tag} series after close: {}", screen_text(shell));
                shot(shell, &format!("real-{tag}-after")).await;
            }
            probe(shell, "real done");
        }
        "hunt-a3" => {
            // L5A-3: Try again on "Couldn't open chapter". A chapter the series doesn't list
            // fails to open (NotFound), then Try again opens it again.
            read(shell, MANGA, "999");
            for _ in 0..20 {
                wait_ms(1000).await;
                if shell.reader.hunt_facts().contains("visible=Some(\"failed\")") {
                    break;
                }
            }
            probe(shell, "a3 failed plate");
            tracing::info!("HUNT a3 text: {}", screen_text(shell));
            shot(shell, "a3-failed").await;
            let shown = shell.reader.hunt_try_again();
            tracing::info!("HUNT a3 pressed Try again, plate shown={shown}");
            wait_ms(3000).await;
            probe(shell, "a3 after Try again");
            shot(shell, "a3-after").await;
        }
        "hunt-a1" => {
            // L5A-1: a read volume opened in Chapters, drawn again when the chapters come again.
            use kaeri_model::reading::{
                ChapterReadState, ChapterRow, ReadingEvent, VolumeGroup,
            };
            read(shell, MANGA, "13");
            wait_ms(5000).await;
            probe(shell, "a1 opened");
            let row = |n: usize, read: bool| ChapterRow {
                id: kaeri_core::books::ChapterId(n.to_string()),
                label: format!("Chapter {n}"), // i18n-skip
                state: if read { ChapterReadState::Read } else { ChapterReadState::Unread },
                number: Some(n.to_string()),
                on_device: false,
                title: None,
                volume: Some(if n <= 6 { "1".into() } else { "2".into() }),
                published: None,
                pages: None,
                new_chapter: false,
                download: None,
                versions: 1,
                bookmarked: false,
                version_label: None,
                read_at: None,
                line: Vec::new(),
                gone_from_source: false,
                parts: 1,
                missing_parts: Vec::new(),
                part_groups: Vec::new(),
                other_group: false,
                group_language: None,
                other_source: None,
            };
            let volumes = vec![
                VolumeGroup {
                    volume: Some("1".into()),
                    title: None,
                    chapters: (1..=6).map(|n| row(n, true)).collect(),
                    all_read: true,
                    flat: false,
                },
                VolumeGroup {
                    volume: Some("2".into()),
                    title: None,
                    chapters: (7..=20).map(|n| row(n, n < 13)).collect(),
                    all_read: false,
                    flat: false,
                },
            ];
            let event = || {
                kaeri_model::event::Event::Reading(ReadingEvent::Chapters {
                    work: kaeri_core::books::WorkId(MANGA.into()),
                    edition: None,
                    volumes: volumes.clone(),
                    failure: None,
                })
            };
            shell.ctx.hub.show_instead(vec![event()]);
            wait_ms(1000).await;
            shell.reader.shot_chrome(true);
            wait_ms(800).await;
            shell.reader.hunt_chapters();
            wait_ms(800).await;
            shell.reader.hunt_chapters_tab();
            wait_ms(1500).await;
            // The volumes again with the popover open, so the list is drawn from them.
            shell.ctx.hub.show_instead(vec![event()]);
            wait_ms(1500).await;
            shot(shell, "a1-chapters").await;
            let pop = shell.reader.hunt_chapters_popover();
            // The list opens on the open chapter; the fold's row is built once the top is in view.
            if let Some(list) = find::<gtk::ListView>(&pop, &|_| true) {
                list.scroll_to(0, gtk::ListScrollFlags::NONE, None);
            }
            wait_ms(1200).await;
            let fold = find::<adw::ExpanderRow>(&pop, &|_| true)
                .or_else(|| find::<adw::ExpanderRow>(shell.reader.widget.upcast_ref(), &|_| true));
            let lists = find::<gtk::ListView>(&pop, &|_| true)
                .and_then(|l| l.model())
                .map(|m| m.n_items());
            tracing::info!(
                "HUNT a1 fold found={} title={:?} popover={} pop_mapped={} list_items={:?}",
                fold.is_some(),
                fold.as_ref().map(|f| f.title()),
                shell.reader.hunt_chapters_open(),
                pop.is_mapped(),
                lists
            );
            if let Some(fold) = fold {
                fold.set_expanded(true);
                wait_ms(1200).await;
                probe(shell, "a1 expanded");
                shot(shell, "a1-expanded").await;
                tracing::info!("HUNT a1 chapters come again");
                shell.ctx.hub.show_instead(vec![event()]);
                wait_ms(2000).await;
                probe(shell, "a1 after redraw");
                shot(shell, "a1-after").await;
            }
        }
        "hunt-a2" => {
            // L5A-2: a folder added again, then Cancel.
            let base = std::env::var("HUNT_DATA").unwrap_or_else(|_| "/tmp".into());
            let dir = format!("{base}/Shelf Box");
            let made = std::process::Command::new("python3")
                .arg("-c")
                .arg("import sys,os,zipfile,zlib,struct\nd=sys.argv[1]\nos.makedirs(d,exist_ok=True)\ndef png():\n  raw=b''.join(b'\\x00'+b'\\x80\\x80\\x80'*8 for _ in range(8))\n  c=lambda t,b: struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b))\n  return b'\\x89PNG\\r\\n\\x1a\\n'+c(b'IHDR',struct.pack('>IIBBBBB',8,8,8,2,0,0,0))+c(b'IDAT',zlib.compress(raw))+c(b'IEND',b'')\nfor n in ('Marsh Lantern v01.cbz','Marsh Lantern v02.cbz'):\n  z=zipfile.ZipFile(os.path.join(d,n),'w')\n  [z.writestr('%03d.png'%i,png()) for i in range(3)]\n  z.close()\nprint(sorted(os.listdir(d)))")
                .arg(&dir)
                .output();
            tracing::info!(
                "HUNT a2 folder made: {:?}",
                made.map(|o| String::from_utf8_lossy(&o.stdout).into_owned()
                    + &String::from_utf8_lossy(&o.stderr))
            );
            let _ = shell.ctx.hub.listen(|event| {
                use kaeri_model::book_sources::BookSourceEvent;
                match event {
                    kaeri_model::event::Event::BookSources(BookSourceEvent::View(view)) => {
                        let folders: Vec<String> = view
                            .folders
                            .iter()
                            .map(|f| {
                                format!(
                                    "{}:{:?}:{:?}:series={}:removed={}",
                                    f.id, f.name, f.scan, f.series, f.removed
                                )
                            })
                            .collect();
                        tracing::info!("HUNT a2 view folders={folders:?}");
                    }
                    kaeri_model::event::Event::CameIn(came) => {
                        tracing::info!(
                            "HUNT a2 came in: {}",
                            format!("{came:?}").chars().take(300).collect::<String>()
                        );
                    }
                    _ => {}
                }
            });
            shell.go(Dest::Read);
            wait_ms(3000).await;
            crate::reading::came_in::hunt_add_folder(&shell.ctx, &dir);
            wait_ms(8000).await;
            shot(shell, "a2-first-answer").await;
            tracing::info!("HUNT a2 first Add: {}", crate::reading::came_in::hunt_folder(true));
            wait_ms(3000).await;
            crate::reading::came_in::hunt_send(
                &shell.ctx,
                kaeri_model::book_sources::BookSourceCommand::Load,
            );
            wait_ms(2000).await;
            tracing::info!("HUNT a2 text after first add: {}", screen_text(shell));
            crate::reading::came_in::hunt_add_folder(&shell.ctx, &dir);
            wait_ms(8000).await;
            shot(shell, "a2-second-answer").await;
            tracing::info!("HUNT a2 second dialog text: {}", screen_text(shell));
            tracing::info!(
                "HUNT a2 second Cancel: {}",
                crate::reading::came_in::hunt_folder(false)
            );
            wait_ms(2000).await;
            shot(shell, "a2-cancelled").await;
            wait_ms(12000).await;
            crate::reading::came_in::hunt_send(
                &shell.ctx,
                kaeri_model::book_sources::BookSourceCommand::Load,
            );
            wait_ms(2000).await;
            tracing::info!("HUNT a2 text 14 s after Cancel: {}", screen_text(shell));
            shot(shell, "a2-after").await;
        }
        "hunt-rsearch" => {
            // Real reading search: HUNT_Q words typed into Read's search.
            let q = std::env::var("HUNT_Q").unwrap_or_else(|_| "reincarnation".into());
            shell.go(Dest::Read);
            wait_ms(15000).await;
            shell.go(Dest::ReadSearch);
            wait_ms(2000).await;
            let entry = find_entry(shell.ctx.window.upcast_ref());
            tracing::info!("HUNT rsearch entry found={}", entry.is_some());
            if let Some(entry) = entry {
                entry.set_text(&q);
                entry.emit_by_name::<()>("activate", &[]);
            }
            for i in 0..6 {
                wait_ms(5000).await;
                tracing::info!("HUNT rsearch {i}: {}", screen_text(shell));
            }
            shot(shell, "rsearch").await;
            // The sources' own hits: Your sources only.
            let mine = find::<gtk::ToggleButton>(shell.ctx.window.upcast_ref(), &|b| {
                b.label().as_deref() == Some("Your sources only")
            });
            tracing::info!("HUNT rsearch mine found={}", mine.is_some());
            if let Some(mine) = mine {
                mine.set_active(true);
            }
            for i in 0..4 {
                wait_ms(6000).await;
                tracing::info!("HUNT rsearch mine {i}: {}", screen_text(shell));
            }
            shot(shell, "rsearch-mine").await;
        }
        "hunt-close" => {
            // What a closed reader keeps, over time.
            read(shell, BIG, "1");
            wait_ms(5000).await;
            for _ in 0..40 {
                shell.reader.pad(ReaderPad::Turn(Direction::Down));
                wait_ms(60).await;
            }
            wait_ms(4000).await;
            probe(shell, "close before");
            shell.reader.close();
            for t in [300u64, 700, 1000, 3000, 10000] {
                wait_ms(t).await;
                probe(shell, &format!("close +{t}"));
            }
        }
        "hunt-mode" => {
            // A mode picked in the open reader: HUNT_MODE="work:chapter:seq", seq of
            // strip|rtl|ltr|auto|end|turn|close separated by '+'.
            let plan = std::env::var("HUNT_MODE").unwrap_or_else(|_| {
                format!("{MANGA}:13:turn+turn+strip+turn+rtl+ltr+strip+end+rtl+auto")
            });
            let mut it = plan.split(':');
            let work = format!("{}:{}", it.next().unwrap_or("a"), it.next().unwrap_or("0"));
            let chapter = it.next().unwrap_or("1").to_owned();
            let seq = it.next().unwrap_or("").to_owned();
            if std::env::var("REAL").is_ok() {
                shell.go(Dest::Read);
                wait_ms(15000).await;
            }
            series(shell, &work);
            wait_ms(6000).await;
            read(shell, &work, &chapter);
            for i in 0..30 {
                wait_ms(2000).await;
                let facts = shell.reader.hunt_facts();
                if !facts.contains("waiting=Some(Chapter)") && i >= 3 {
                    break;
                }
            }
            probe(shell, "mode opened");
            shot(shell, "mode-0-open").await;
            for (n, step) in seq.split('+').enumerate() {
                match step {
                    "turn" => shell.reader.pad(ReaderPad::Turn(Direction::Down)),
                    "end" => shell.reader.hunt_key(gtk::gdk::Key::End),
                    "close" => shell.reader.close(),
                    "off" => net(shell, true),
                    "on" => net(shell, false),
                    "down" => {
                        for _ in 0..6 {
                            shell.reader.hunt_key(gtk::gdk::Key::Page_Down);
                            wait_ms(250).await;
                        }
                    }
                    which => shell.reader.hunt_mode(which),
                }
                for t in [600u64, 2400, 5000] {
                    wait_ms(t).await;
                    probe(shell, &format!("mode {} {step} +{t}", n + 1));
                }
                tracing::info!("HUNT mode {} {step} text: {}", n + 1, screen_text(shell));
                shot(shell, &format!("mode-{}-{step}", n + 1)).await;
            }
            shell.reader.close();
            wait_ms(3000).await;
            probe(shell, "mode closed");
        }
        "hunt-p11" => {
            // iPhone P11: strip, a downloaded chapter, End, on into the next chapter while it
            // loads, then two idle minutes. HUNT_P11="work:chapter".
            let plan = std::env::var("HUNT_P11").unwrap_or_else(|_| format!("{STRIP}:1"));
            let (work, chapter) = plan.rsplit_once(':').unwrap_or((STRIP, "1"));
            let (work, chapter) = (work.to_owned(), chapter.to_owned());
            let _ = shell.ctx.hub.listen(|event| {
                if let kaeri_model::event::Event::Reading(kaeri_model::reading::ReadingEvent::Chapters {
                    volumes, ..
                }) = event
                {
                    let marked: Vec<String> = volumes
                        .iter()
                        .flat_map(|v| v.chapters.iter())
                        .filter(|r| {
                            !matches!(r.state, kaeri_model::reading::ChapterReadState::Unread) || r.download.is_some()
                        })
                        .take(40)
                        .map(|r| format!("{}:{:?}:{:?}", r.id, r.state, r.download))
                        .collect();
                    tracing::info!("HUNT p11 chapters marked={marked:?}");
                }
            });
            if std::env::var("REAL").is_ok() {
                shell.go(Dest::Read);
                wait_ms(15000).await;
            }
            series(shell, &work);
            wait_ms(10000).await;
            tracing::info!("HUNT p11 series before: {}", screen_text(shell));
            send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(&work),
                    pick: kaeri_model::reading::DownloadPick::Chapters {
                        ids: vec![kaeri_core::books::ChapterId::new(&chapter)],
                    },
                },
            );
            for i in 0..36 {
                wait_ms(5000).await;
                if i % 6 == 5 {
                    files_line();
                }
            }
            shell.go(Dest::ReadDownloads);
            wait_ms(3000).await;
            tracing::info!("HUNT p11 downloads: {}", screen_text(shell));
            read(shell, &work, &chapter);
            wait_ms(8000).await;
            shell.reader.hunt_mode("strip");
            wait_ms(4000).await;
            probe(shell, "p11 opened strip");
            shell.reader.hunt_key(gtk::gdk::Key::End);
            wait_ms(4000).await;
            probe(shell, "p11 end");
            for _ in 0..2 {
                for _ in 0..6 {
                    shell.reader.hunt_key(gtk::gdk::Key::Page_Down);
                    wait_ms(250).await;
                }
                wait_ms(1500).await;
            }
            probe(shell, "p11 crossed");
            shot(shell, "p11-crossed").await;
            for s in 1..=12 {
                wait_ms(10000).await;
                probe(shell, &format!("p11 idle {}s", s * 10));
            }
            shot(shell, "p11-idle").await;
            shell.reader.close();
            wait_ms(3000).await;
            series(shell, &work);
            wait_ms(10000).await;
            tracing::info!("HUNT p11 series after: {}", screen_text(shell));
            shot(shell, "p11-series-after").await;
        }
        "hunt-x-kb" => {
            serve_requests(shell);
            shell.go(Dest::Read);
            for i in 0..58 {
                wait_ms(5000).await;
                probe(shell, &format!("x-kb {i}"));
            }
        }
        "hunt-x-kb-reader" => {
            serve_requests(shell);
            series(shell, MANGA);
            for i in 0..58 {
                wait_ms(5000).await;
                probe(shell, &format!("x-kb-reader {i}"));
            }
        }
        "hunt-sleep" => {
            for i in 0..22 {
                wait_ms(5000).await;
                probe(shell, &format!("sleep {i}"));
            }
        }
        _ => {}
    }
}


// ---------------------------------------------------------------------------------------------
// Round 6: a drive read from HUNT_DO, steps split by ';', words by spaces.

fn key_named(name: &str) -> Option<gtk::gdk::Key> {
    gtk::gdk::Key::from_name(name)
}

/// Every mapped widget whose own text (or a descendant's) contains `text`, deepest first.
fn click_text(shell: &Rc<Shell>, text: &str, nth: usize) -> String {
    act_on_text(shell, text, nth, false)
}

/// The keyboard on the widget `click_text` would click, as Tab would leave it there.
fn focus_text(shell: &Rc<Shell>, text: &str, nth: usize) -> String {
    act_on_text(shell, text, nth, true)
}

fn act_on_text(shell: &Rc<Shell>, text: &str, nth: usize, focus_only: bool) -> String {
    fn walk(w: &gtk::Widget, text: &str, out: &mut Vec<gtk::Widget>) {
        if !w.is_mapped() {
            return;
        }
        let mut c = w.first_child();
        while let Some(n) = c {
            walk(&n, text, out);
            c = n.next_sibling();
        }
        let actionable = w.is::<gtk::Button>()
            || w.is::<gtk::FlowBoxChild>()
            || w.is::<gtk::ListBoxRow>()
            || w.is::<gtk::ToggleButton>()
            || w.is::<gtk::CheckButton>()
            || w.is::<gtk::MenuButton>();
        if !actionable {
            return;
        }
        let mut t = Vec::new();
        labels(w, &mut t);
        if let Some(b) = w.downcast_ref::<gtk::Button>()
            && let Some(l) = b.label()
        {
            t.push(l.to_string());
        }
        if let Some(tip) = w.tooltip_text() {
            t.push(tip.to_string());
        }
        if t.iter().any(|x| x.contains(text)) {
            out.push(w.clone());
        }
    }
    let mut found = Vec::new();
    walk(shell.ctx.window.upcast_ref(), text, &mut found);
    let n = found.len();
    let Some(w) = found.get(nth).cloned() else {
        return format!("no widget with {text:?} (found {n})");
    };
    let what = describe(&w);
    if focus_only {
        shell.ctx.window.set_focus_visible(true);
        let took = w.grab_focus();
        return format!("focused {nth} of {n} ({took}): {what}");
    }
    if let Some(b) = w.downcast_ref::<gtk::Button>() {
        b.emit_clicked();
    } else if let Some(c) = w.downcast_ref::<gtk::FlowBoxChild>() {
        c.activate();
    } else if let Some(r) = w.downcast_ref::<gtk::ListBoxRow>() {
        r.activate();
    } else if let Some(c) = w.downcast_ref::<gtk::CheckButton>() {
        c.set_active(!c.is_active());
    } else if let Some(m) = w.downcast_ref::<gtk::MenuButton>() {
        m.popup();
    } else {
        w.activate();
    }
    format!("clicked {nth} of {n}: {what}")
}

fn listen_all(shell: &Rc<Shell>) {
    let _ = shell.ctx.hub.listen(|event| {
        use kaeri_model::event::Event;
        let line = match event {
            Event::BookSources(e) => format!("{e:?}"),
            Event::CameIn(e) => format!("{e:?}"),
            Event::Reading(e) => {
                let s = format!("{e:?}");
                if s.starts_with("Failed") || s.starts_with("Download") || s.starts_with("Notice") {
                    s
                } else {
                    return;
                }
            }
            _ => return,
        };
        tracing::info!("HUNT ev {}", line.chars().take(400).collect::<String>());
    });
}

async fn do_steps(shell: &Rc<Shell>) {
    let plan = std::env::var("HUNT_DO").unwrap_or_default();
    if std::env::var("HUNT_EVLOG").is_ok() {
        listen_all(shell);
    }
    let steps: Vec<String> =
        plan.split(';').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
    let mut i = 0;
    let mut loops: Vec<(usize, u32)> = Vec::new();
    while i < steps.len() {
        let step = steps[i].clone();
        let w: Vec<&str> = step.split(' ').collect();
        let arg = |n: usize| w.get(n).copied().unwrap_or("");
        let num = |n: usize, d: u64| w.get(n).and_then(|x| x.parse::<u64>().ok()).unwrap_or(d);
        tracing::info!("HUNT do [{i}] {step}");
        match arg(0) {
            "go" => shell.go(match arg(1) {
                "home" => Dest::Home,
                "library" => Dest::ReadLibrary,
                "downloads" => Dest::ReadDownloads,
                "sources" => Dest::ReadSources,
                "search" => Dest::ReadSearch,
                "updates" => Dest::Updates,
                "calendar" => Dest::Calendar,
                "settings" => Dest::Settings,
                _ => Dest::Read,
            }),
            "series" => series(shell, arg(1)),
            "read" => read(shell, arg(1), arg(2)),
            "wait" => wait_ms(num(1, 1000)).await,
            "waitopen" => {
                let t0 = std::time::Instant::now();
                for k in 0..num(1, 60) {
                    wait_ms(1000).await;
                    let f = shell.reader.hunt_facts();
                    if !f.contains("waiting=Some(Chapter)") && k >= 2 {
                        break;
                    }
                }
                tracing::info!("HUNT do opened after {}ms", t0.elapsed().as_millis());
            }
            "probe" => probe(shell, &format!("do {}", w[1..].join(" "))),
            "text" => tracing::info!("HUNT text {}: {}", arg(1), screen_text(shell)),
            "shot" => shot(shell, arg(1)).await,
            "width" => width_need(shell, arg(1)),
            "frames" => frames(shell, arg(1)),
            "key" => {
                let n = num(2, 1);
                for _ in 0..n {
                    if let Some(k) = key_named(arg(1)) {
                        shell.reader.hunt_key(k);
                    }
                    wait_ms(num(3, 150)).await;
                }
            }
            "turn" | "back" => {
                let d = if arg(0) == "turn" { Direction::Down } else { Direction::Up };
                for _ in 0..num(1, 1) {
                    shell.reader.pad(ReaderPad::Turn(d));
                    wait_ms(num(2, 300)).await;
                }
            }
            "mode" => shell.reader.hunt_mode(arg(1)),
            "close" => shell.reader.close(),
            "off" => net(shell, true),
            "on" => net(shell, false),
            "dl" => send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(arg(1)),
                    pick: kaeri_model::reading::DownloadPick::NextUnread { n: num(2, 1) as u32 },
                },
            ),
            "dlch" => send(
                shell,
                kaeri_model::reading::ReadingCommand::Download {
                    work: kaeri_core::books::WorkId::new(arg(1)),
                    pick: kaeri_model::reading::DownloadPick::Chapters {
                        ids: w[2..].iter().map(|c| kaeri_core::books::ChapterId::new(*c)).collect(),
                    },
                },
            ),
            "addfolder" => crate::reading::came_in::hunt_add_folder(&shell.ctx, &w[1..].join(" ")),
            "folder" => tracing::info!(
                "HUNT do folder: {}",
                crate::reading::came_in::hunt_folder(arg(1) == "keep")
            ),
            "srcload" => crate::reading::came_in::hunt_send(
                &shell.ctx,
                kaeri_model::book_sources::BookSourceCommand::Load,
            ),
            "click" => {
                let nth = arg(1).parse::<usize>().ok();
                let text = if nth.is_some() { w[2..].join(" ") } else { w[1..].join(" ") };
                tracing::info!("HUNT do click: {}", click_text(shell, &text, nth.unwrap_or(0)));
            }
            "focus" => {
                let nth = arg(1).parse::<usize>().ok();
                let text = if nth.is_some() { w[2..].join(" ") } else { w[1..].join(" ") };
                tracing::info!("HUNT do focus: {}", focus_text(shell, &text, nth.unwrap_or(0)));
            }
            "send" => crate::reading::export::open(&shell.ctx, &kaeri_core::books::WorkId::new(arg(1))),
            "chapters" => shell.reader.hunt_chapters(),
            "chapterstab" => shell.reader.hunt_chapters_tab(),
            "tryagain" => tracing::info!("HUNT do try again: {}", shell.reader.hunt_try_again()),
            "files" => files_line(),
            "size" => {
                let (a, b) = (num(1, 1280) as i32, num(2, 800) as i32);
                shell.ctx.window.set_default_size(a, b);
            }
            "repeat" => loops.push((i, num(1, 2) as u32)),
            "end" => {
                if let Some((start, left)) = loops.pop() {
                    if left > 1 {
                        loops.push((start, left - 1));
                        i = start + 1;
                        continue;
                    }
                }
            }
            "abort" => {
                tracing::info!("HUNT do aborting now");
                std::process::abort();
            }
            "quit" => {
                if let Some(app) = shell.ctx.window.application() {
                    app.quit();
                }
            }
            other => tracing::warn!("HUNT do: unknown step {other}"),
        }
        i += 1;
    }
    tracing::info!("HUNT do done");
}

/// A made-up library of `HUNT_BIGLIB` series (default none), kept as AniList-shaped book works
/// from the fixture template with new ids and invented titles, spread over the reading lists,
/// with a place in every seventh.
pub fn biglib(store: &kaeri_store::Store, art: &str, now: i64) {
    let Some(n) = std::env::var("HUNT_BIGLIB").ok().and_then(|v| v.parse::<u32>().ok()) else {
        return;
    };
    let templates = kaeri_testkit::fixtures::books();
    let words = [
        "Lantern", "Harbor", "Willow", "Copper", "Meadow", "Quiet", "Ember", "Salt", "Paper",
        "Tide", "Glass", "Moss", "Cinder", "Hollow", "Silver", "Kite",
    ];
    let lists = ["reading", "planning", "completed", "paused", "dropped", "rereading"];
    let t0 = std::time::Instant::now();
    for i in 0..n {
        let template = &templates[(i as usize) % templates.len()];
        let mut v = template.clone();
        let id = 9_100_000 + i as i64;
        let name = format!(
            "{} {} of the {} {}",
            words[(i % 16) as usize],
            words[((i / 16) % 16) as usize],
            words[((i / 256) % 16) as usize],
            i
        );
        v["id"] = serde_json::json!(id);
        v["idMal"] = serde_json::Value::Null;
        v["title"] = serde_json::json!({ "romaji": name, "english": name, "native": null });
        v["synonyms"] = serde_json::json!([]);
        let json = v.to_string().replace(kaeri_testkit::fixtures::ART_HOST, art);
        let Ok(book) = serde_json::from_str::<kaeri_meta::anilist::books::Book>(&json) else {
            tracing::warn!("HUNT biglib {i}: the template did not decode");
            continue;
        };
        let work = format!("a:{id}");
        let titles = serde_json::json!({
            "romaji": book.title.romaji, "english": book.title.english,
            "native": book.title.native, "synonyms": book.synonyms,
        });
        let row = kaeri_store::book_catalog::BookWork {
            work: work.clone(),
            anilist_id: Some(book.id.get()),
            medium: book.medium().as_str().to_owned(),
            format: Some(book.reading_format().as_str().to_owned()),
            origin: book.origin().map(|o| o.code().to_owned()),
            titles: titles.to_string(),
            cover: book.cover_image.best().map(str::to_owned),
            status: book
                .status
                .and_then(|s| serde_json::to_value(s).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            chapters_total: book.chapters,
            volumes_total: book.volumes,
            meta: Some(serde_json::json!({ "book": book, "detail_at": null }).to_string()),
            fetched_at: now,
        };
        if let Err(e) = store.put_book_work(&row) {
            tracing::warn!("HUNT biglib put {i}: {e}");
            continue;
        }
        let list = lists[(i as usize * 7 + i as usize / 3) % lists.len()];
        if let Err(e) = store.set_read_status(&work, Some(list), now - i64::from(i)) {
            tracing::warn!("HUNT biglib status {i}: {e}");
        }
        if i % 7 == 0 {
            let mut ev = kaeri_store::reading::ReadEvent::new(&work, format!("{}", 1 + i % 40), now - i64::from(i) * 60);
            ev.page = Some(3);
            ev.of_pages = Some(20);
            if let Err(e) = store.record_read(&ev, Vec::new()) {
                tracing::warn!("HUNT biglib read {i}: {e}");
            }
        }
    }
    tracing::info!("HUNT biglib kept {n} series in {}ms", t0.elapsed().as_millis());
}
