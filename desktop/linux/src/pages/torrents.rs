//! Torrents: the optional proxy their traffic can go through. Turning the proxy on, or editing
//! it, opens a form that checks the proxy before saving it. The same words as the other apps.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_model::{
    command::Command,
    event::Event,
    proxy::{ProxyView, TorrentProxy},
    sources::ProxyStatus,
    stores::Secret,
};

use gtk::glib;
use kaeri_i18n::t;

use crate::window::Ctx;

/// The privacy line, under the form's title.
pub fn exposed() -> String {
    t::torrents_risk()
}
/// What the proxy check does.
pub fn check_note() -> String {
    t::torrents_check_note()
}
pub fn off_note() -> String {
    t::torrents_off_detail()
}
/// How torrent traffic leaves now, under the group's title while torrents are on.
pub fn on_note(proxy: Option<&ProxyView>) -> String {
    match proxy {
        Some(proxy) => t::torrents_summary_proxy_at(&proxy.host, &proxy.port.to_string()),
        None => t::torrents_summary_direct(),
    }
}

/// The proxy check's last word: the note under "Proxy check", the word beside it, and whether
/// it failed. `None` when there is nothing to show (no check, or no proxy to check).
pub fn check_words(status: Option<ProxyStatus>) -> Option<(String, String, bool)> {
    match status? {
        ProxyStatus::Passed => Some((String::new(), t::torrents_check_passed(), false)),
        ProxyStatus::Checking => Some((String::new(), t::torrents_check_running(), false)),
        ProxyStatus::Failed => {
            Some((t::torrents_check_failed_line(), t::torrents_check_failed(), true))
        }
        ProxyStatus::Direct => None,
    }
}

/// The form's fields before they are sent. A password left empty keeps the one kept.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Draft {
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
}

impl Draft {
    /// The proxy as the engine takes it, when the fields make one.
    pub fn proxy(&self) -> Option<TorrentProxy> {
        let host = self.host.trim();
        let port: u16 = self.port.trim().parse().ok().filter(|p| *p > 0)?;
        (!host.is_empty()).then(|| TorrentProxy {
            host: host.to_owned(),
            port,
            user: Some(self.user.trim().to_owned()).filter(|u| !u.is_empty()),
            password: Some(Secret(self.password.clone())).filter(|p| !p.0.is_empty()),
        })
    }

    /// Whether it can be sent: a host and a port.
    pub fn complete(&self) -> bool {
        self.proxy().is_some()
    }
}

/// The proxy as the engine last said it.
pub fn current(ctx: &Ctx) -> kaeri_model::proxy::TorrentView {
    ctx.hub.known.borrow().settings.as_ref().map(|v| (*v.torrent).clone()).unwrap_or_default()
}

/// What the proxy form is for: routing torrents through a proxy, or editing the one set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Route,
    EditProxy,
}

/// Edit Proxy's line over its form.
pub fn edit_note() -> String {
    t::torrents_proxy_keeps_saved()
}
/// Under the password, while a proxy's password is kept.
pub fn password_kept() -> String {
    t::torrents_password_keep()
}
/// While a proxy is checked before it is saved.
pub fn checking_proxy() -> String {
    t::torrents_checking_proxy()
}

/// Once the check passes and the proxy is saved, before the form closes.
fn saved() -> String {
    t::torrents_proxy_saved()
}

/// Why a proxy was not saved: the check failed, and the saved proxy stays.
fn not_saved() -> String {
    t::torrents_proxy_not_saved(&t::torrents_check_not_made_proxy())
}

/// The form that routes torrents through a proxy: its address, and the check's note.
/// `cancelled` runs when it closes unsent.
pub fn setup(ctx: &Rc<Ctx>, cancelled: Rc<dyn Fn()>) -> adw::Dialog {
    form(ctx, Purpose::Route, cancelled)
}

/// Edit Proxy: the proxy set, filled in but for its password, which left empty stays as it
/// is. Saved only once the check passes on it.
pub fn edit_proxy(ctx: &Rc<Ctx>) -> adw::Dialog {
    form(ctx, Purpose::EditProxy, Rc::new(|| {}))
}

/// Proxy checks, one at a time. The engine runs each off its loop and its verdicts carry
/// no number, so two in flight could answer in either order. Only one is asked at once; the
/// others wait their turn, and a waiting check whose form has closed is dropped.
struct Checks<T> {
    /// The form whose check the engine is running.
    asked: Option<u64>,
    waiting: std::collections::VecDeque<(u64, T)>,
}

impl<T> Default for Checks<T> {
    fn default() -> Self {
        Self { asked: None, waiting: std::collections::VecDeque::new() }
    }
}

impl<T> Checks<T> {
    /// `form` wants `check` made: given back when it can be asked now, kept for later if not.
    fn ask(&mut self, form: u64, check: T) -> Option<T> {
        if self.asked.is_some() {
            self.waiting.push_back((form, check));
            return None;
        }
        self.asked = Some(form);
        Some(check)
    }

    /// `form` has closed: its waiting check is not asked. One already asked runs out, and its
    /// verdict reaches no one.
    fn drop_form(&mut self, form: u64) {
        self.waiting.retain(|(at, _)| *at != form);
    }

    /// A verdict came: whose check it was on, and the next check to ask, now asked.
    fn answered(&mut self) -> (Option<u64>, Option<T>) {
        let owner = self.asked.take();
        let next = self.waiting.pop_front().map(|(form, check)| {
            self.asked = Some(form);
            check
        });
        (owner, next)
    }
}

type Check = TorrentProxy;
type Verdict = Rc<dyn Fn(ProxyStatus)>;

thread_local! {
    static CHECKS: std::cell::RefCell<Checks<Check>> = std::cell::RefCell::default();
    /// The open forms waiting on a verdict, by number.
    static WAITING: std::cell::RefCell<Vec<(u64, Verdict)>> = const { std::cell::RefCell::new(Vec::new()) };
    static NEXT_FORM: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static LISTENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// For a shot: forget checks held unanswered, so the next form's is asked at once.
#[cfg(feature = "fixtures")]
pub fn forget_checks() {
    CHECKS.with_borrow_mut(|checks| *checks = Checks::default());
}

/// Ask the engine to check `proxy` for `form` without saving it, once the checks
/// before it have answered. `verdict` hears its answer, unless the form closes first.
fn ask_check(ctx: &Rc<Ctx>, form: u64, proxy: TorrentProxy, verdict: Verdict) {
    if !LISTENING.replace(true) {
        let hub = Rc::downgrade(&ctx.hub);
        ctx.hub.listen(move |event| {
            let Event::ProxyChecked(status) = event else { return };
            let (owner, next) = CHECKS.with_borrow_mut(Checks::answered);
            let heard = owner.and_then(|owner| {
                WAITING.with_borrow_mut(|waiting| {
                    let at = waiting.iter().position(|(form, _)| *form == owner)?;
                    Some(waiting.remove(at).1)
                })
            });
            if let (Some(proxy), Some(hub)) = (next, hub.upgrade()) {
                hub.send(Command::CheckTorrentProxy { proxy });
            }
            if let Some(heard) = heard {
                heard(*status);
            }
        });
    }
    WAITING.with_borrow_mut(|waiting| waiting.push((form, verdict)));
    if let Some(proxy) = CHECKS.with_borrow_mut(|checks| checks.ask(form, proxy)) {
        ctx.hub.send(Command::CheckTorrentProxy { proxy });
    }
}

/// `form` has closed: it hears no verdict, and its waiting check is not asked.
fn forget_form(form: u64) {
    WAITING.with_borrow_mut(|waiting| waiting.retain(|(at, _)| *at != form));
    CHECKS.with_borrow_mut(|checks| checks.drop_form(form));
}

/// Where a form's check stands, in the line under its fields.
enum Standing {
    Idle,
    Checking,
    Failed(String),
    Saved,
}

fn form(ctx: &Rc<Ctx>, purpose: Purpose, cancelled: Rc<dyn Fn()>) -> adw::Dialog {
    let editing = purpose == Purpose::EditProxy;
    let known = current(ctx);
    let draft = Rc::new(std::cell::RefCell::new(Draft {
        host: known.proxy.as_ref().map(|p| p.host.clone()).unwrap_or_default(),
        port: known.proxy.as_ref().map(|p| p.port.to_string()).unwrap_or_default(),
        user: known.proxy.as_ref().and_then(|p| p.user.clone()).unwrap_or_default(),
        password: String::new(),
    }));

    let title = match purpose {
        Purpose::Route => t::torrents_proxy_route(),
        Purpose::EditProxy => t::torrents_linux_edit_proxy_title(),
    };
    let dialog = adw::Dialog::builder().title(title).content_width(480).build();
    let header = adw::HeaderBar::builder().show_end_title_buttons(false).build();
    let cancel = gtk::Button::with_label(&t::action_cancel());
    let send = gtk::Button::with_label(&t::action_save());
    send.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&send);

    let page = adw::PreferencesPage::new();
    page.set_description(&if editing { edit_note() } else { exposed() });
    // What the viewer can change, locked while a check runs.
    let mut fields: Vec<adw::PreferencesGroup> = Vec::new();
    let proxy = adw::PreferencesGroup::builder().title(t::torrents_proxy_route()).build();
    let field = |title: &str, text: &str| {
        let row = adw::EntryRow::builder().title(title).text(text).build();
        proxy.add(&row);
        row
    };
    let host = field(&t::torrents_proxy_host(), &draft.borrow().host);
    let port = field(&t::torrents_proxy_port(), &draft.borrow().port);
    port.set_input_purpose(gtk::InputPurpose::Digits);
    let user = field(&t::torrents_proxy_user(), &draft.borrow().user);
    let password = adw::PasswordEntryRow::builder().title(t::torrents_proxy_password()).build();
    if known.proxy.is_some() {
        proxy.set_description(Some(&password_kept()));
    }
    proxy.add(&password);

    // The check on a proxy before it is saved: under the fields, in the one place, so nothing
    // above it moves as it comes and goes.
    let status = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("mark");
    dot.set_valign(gtk::Align::Start);
    dot.set_margin_top(7);
    let words = gtk::Label::new(None);
    words.set_xalign(0.0);
    words.set_wrap(true);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Start);
    spinner.set_valign(gtk::Align::Start);
    spinner.set_margin_top(2);
    spinner.set_visible(false);
    dot.set_visible(false);
    status.append(&spinner);
    status.append(&dot);
    status.append(&words);
    let note = adw::PreferencesGroup::builder().description(check_note()).build();
    page.add(&proxy);
    let status_group = adw::PreferencesGroup::new();
    status_group.add(&status);
    // Two lines kept for it, so neither the note nor the dialog
    // moves as the check comes and goes.
    status.set_size_request(-1, 44);
    page.add(&status_group);
    page.add(&note);
    fields.push(proxy.clone());

    let busy = Rc::new(std::cell::Cell::new(false));
    let show: Rc<dyn Fn(Standing)> = Rc::new({
        let (status, spinner, dot, words, fields, send, busy, draft) = (
            status_group.clone(),
            spinner.clone(),
            dot.clone(),
            words.clone(),
            fields,
            send.clone(),
            Rc::clone(&busy),
            Rc::clone(&draft),
        );
        move |standing: Standing| {
            let checking = matches!(standing, Standing::Checking | Standing::Saved);
            busy.set(checking);
            // Locked while the check runs, so the verdict is on what the fields show.
            for group in &fields {
                group.set_sensitive(!checking);
            }
            send.set_sensitive(!checking && draft.borrow().complete());
            spinner.set_visible(matches!(standing, Standing::Checking));
            dot.set_visible(matches!(standing, Standing::Failed(_) | Standing::Saved));
            for each in ["water", "vermilion"] {
                dot.remove_css_class(each);
            }
            let said = match standing {
                Standing::Idle => {
                    words.set_label("");
                    return;
                }
                Standing::Checking => checking_proxy(),
                Standing::Failed(why) => {
                    dot.add_css_class("vermilion");
                    why
                }
                Standing::Saved => {
                    dot.add_css_class("water");
                    saved()
                }
            };
            words.set_label(&said);
            status
                .upcast_ref::<gtk::Widget>()
                .announce(&said, gtk::AccessibleAnnouncementPriority::High);
        }
    });

    // Save only once the fields make a proxy.
    let refresh = {
        let (draft, send, busy) = (Rc::clone(&draft), send.clone(), Rc::clone(&busy));
        move || send.set_sensitive(!busy.get() && draft.borrow().complete())
    };
    let refresh: Rc<dyn Fn()> = Rc::new(refresh);
    refresh();
    let typed = |row: &adw::EntryRow, set: fn(&mut Draft, String)| {
        let (draft, refresh, show) = (Rc::clone(&draft), Rc::clone(&refresh), Rc::clone(&show));
        row.connect_changed(move |row| {
            set(&mut draft.borrow_mut(), row.text().into());
            // A failure was on what the fields said before.
            show(Standing::Idle);
            refresh();
        });
    };
    typed(&host, |d, t| d.host = t);
    typed(&port, |d, t| d.port = t);
    typed(&user, |d, t| d.user = t);
    typed(password.upcast_ref::<adw::EntryRow>(), |d, t| d.password = t);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    dialog.set_child(Some(&view));
    dialog.set_default_widget(Some(&send));

    let sent = Rc::new(std::cell::Cell::new(false));
    // This form's number in the line of checks, let go as it closes.
    let number = NEXT_FORM.get();
    NEXT_FORM.set(number + 1);
    // Why a check couldn't be made comes just after its verdict, and shows here under the
    // failure, not as a toast, for as long as the form is open.
    let claim = ctx.claim_alerts({
        let (words, dot) = (words.clone(), dot.clone());
        move |msg, text| {
            let not_checked = matches!(msg, kaeri_model::Msg::NoticeProxyNotChecked { .. });
            if !not_checked || !dot.has_css_class("vermilion") {
                return false;
            }
            let said = format!("{} {text}.", words.label()); // i18n-skip
            words.set_label(&said);
            words
                .upcast_ref::<gtk::Widget>()
                .announce(&format!("{text}."), gtk::AccessibleAnnouncementPriority::High);
            true
        }
    });
    dialog.connect_closed({
        let (sent, cancelled) = (Rc::clone(&sent), Rc::clone(&cancelled));
        let ctx = Rc::downgrade(ctx);
        move |_| {
            forget_form(number);
            if let Some(ctx) = ctx.upgrade() {
                ctx.release_alerts(claim);
            }
            if !sent.get() {
                cancelled();
            }
        }
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    let ctx_weak = Rc::downgrade(ctx);
    send.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            let Some(ctx) = ctx_weak.upgrade() else { return };
            let draft = draft.borrow().clone();
            let Some(proxy) = draft.proxy() else { return };
            if busy.get() {
                return;
            }
            // Saved, and closed after `linger`, so a saved proxy is seen to be.
            let save: Rc<dyn Fn(u64)> = Rc::new({
                let (ctx, dialog, sent) = (Rc::clone(&ctx), dialog.clone(), Rc::clone(&sent));
                let proxy = proxy.clone();
                move |linger| {
                    if sent.replace(true) {
                        return;
                    }
                    ctx.hub.send(Command::SetTorrentProxy { proxy: Some(proxy.clone()) });
                    let dialog = dialog.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(linger),
                        move || {
                            dialog.close();
                        },
                    );
                }
            });
            // Checked first: only a pass saves, and a failure saves nothing.
            show(Standing::Checking);
            let verdict: Verdict = Rc::new({
                let show = Rc::clone(&show);
                let save = Rc::clone(&save);
                #[cfg(feature = "fixtures")]
                let ctx = Rc::downgrade(&ctx);
                move |status: ProxyStatus| {
                    if status == ProxyStatus::Passed {
                        show(Standing::Saved);
                        // A shot's answer holds the saved form still to be seen.
                        #[cfg(feature = "fixtures")]
                        let linger = if ctx
                            .upgrade()
                            .is_some_and(|ctx| ctx.hub.check_answer.borrow().is_some())
                        {
                            60_000
                        } else {
                            900
                        };
                        #[cfg(not(feature = "fixtures"))]
                        let linger = 900;
                        save(linger);
                    } else {
                        show(Standing::Failed(not_saved()));
                    }
                }
            });
            ask_check(&ctx, number, proxy, verdict);
        }
    });
    dialog.present(Some(&ctx.window));
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proxy_needs_its_host_and_a_port() {
        let mut draft = Draft::default();
        assert!(!draft.complete());
        draft.host = " proxy.example ".into();
        draft.port = "0".into();
        assert!(!draft.complete(), "port 0 is no port");
        draft.port = "1080".into();
        let proxy = draft.proxy().expect("a proxy");
        assert_eq!((proxy.host.as_str(), proxy.port), ("proxy.example", 1080));
        assert_eq!(proxy.user, None);
        assert!(proxy.password.is_none(), "an empty password keeps the kept one");
    }

    #[test]
    fn one_check_runs_at_a_time_and_the_next_waits_its_turn() {
        let mut checks = Checks::default();
        assert_eq!(checks.ask(1, "a"), Some("a"));
        assert_eq!(checks.ask(2, "b"), None, "waits for a's verdict");
        assert_eq!(checks.answered(), (Some(1), Some("b")));
        assert_eq!(checks.answered(), (Some(2), None));
        assert_eq!(checks.ask(3, "c"), Some("c"), "nothing in flight, asked at once");
    }

    #[test]
    fn a_check_cancelled_then_saved_again_hears_only_its_own_verdict() {
        let mut checks = Checks::default();
        // Form 1 asks, and closes before the engine answers; form 2 asks the same settings.
        assert_eq!(checks.ask(1, "a"), Some("a"));
        checks.drop_form(1);
        assert_eq!(checks.ask(2, "b"), None, "b waits, so a's verdict cannot answer it");
        // a's verdict reaches its closed form, then b is asked.
        assert_eq!(checks.answered(), (Some(1), Some("b")));
        assert_eq!(checks.answered(), (Some(2), None));
    }

    #[test]
    fn a_waiting_check_is_dropped_when_its_form_closes() {
        let mut checks = Checks::default();
        assert_eq!(checks.ask(1, "a"), Some("a"));
        assert_eq!(checks.ask(2, "b"), None);
        assert_eq!(checks.ask(3, "c"), None);
        checks.drop_form(2);
        assert_eq!(checks.answered(), (Some(1), Some("c")), "b is never asked");
        assert_eq!(checks.answered(), (Some(3), None));
    }

    #[test]
    fn a_failed_check_says_nothing_was_saved() {
        assert_eq!(not_saved(), "Not saved. The proxy check failed, so the saved proxy stays.");
        assert_eq!(saved(), "Proxy saved");
    }

    #[test]
    fn the_check_row_shows_only_with_a_proxy_to_check() {
        assert_eq!(
            check_words(Some(ProxyStatus::Passed)).map(|w| (w.1, w.2)),
            Some(("Passed".into(), false))
        );
        assert_eq!(
            check_words(Some(ProxyStatus::Failed)).map(|w| (w.1, w.2)),
            Some(("Failed".into(), true))
        );
        assert!(check_words(Some(ProxyStatus::Direct)).is_none());
        assert!(check_words(None).is_none());
    }
}
