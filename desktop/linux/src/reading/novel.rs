//! A novel in WebKitGTK 6, loaded by name when the first novel opens.
//!
//! The engine serves the book: every request to `kaeri-book://` is answered by it, with the
//! policy headers it chose. The page is paginated by the engine's script, injected at the start
//! of every document, which talks back through the `kaeri` message handler with JSON lines. The
//! view can load nothing else: any navigation off the book scheme is refused, and the web view
//! runs in an ephemeral session.
//!
//! WebKitGTK is not linked. Where it is missing, or its sandbox cannot start, `NovelView::new`
//! says so and the reader shows that the chapter cannot open here.

use std::{
    cell::Cell,
    ffi::{CStr, CString, c_char, c_int, c_void},
    rc::Rc,
    sync::OnceLock,
};

use glib::{gobject_ffi, translate::*};
use gtk::{gio, glib, prelude::*};
use kaeri_core::books::ChapterId;
use kaeri_model::reader::{NovelOpenView, NovelPos, ReaderCommand, TurnHow};
use libloading::Library;

use crate::window::Ctx;

/// Room over the text for the reader's header bar: its 46 px and the reader board's gap of 32
/// to the first line (its Linux novel frame). The engine's top is for a Mac window, whose title
/// bar is thinner. The foot is the engine's, which counts this group already.
const HEAD_ROOM: u32 = 78;

type Object = *mut gobject_ffi::GObject;

struct Api {
    _webkit: Library,
    _jsc: Library,
    _soup: Library,
    view_type: unsafe extern "C" fn() -> glib::ffi::GType,
    session_ephemeral: unsafe extern "C" fn() -> Object,
    context_new: unsafe extern "C" fn() -> Object,
    register_scheme: unsafe extern "C" fn(
        Object,
        *const c_char,
        unsafe extern "C" fn(Object, *mut c_void),
        *mut c_void,
        Option<unsafe extern "C" fn(*mut c_void)>,
    ),
    request_uri: unsafe extern "C" fn(Object) -> *const c_char,
    request_finish_with: unsafe extern "C" fn(Object, Object),
    request_finish_error: unsafe extern "C" fn(Object, *mut glib::ffi::GError),
    response_new: unsafe extern "C" fn(Object, i64) -> Object,
    response_status: unsafe extern "C" fn(Object, u32, *const c_char),
    response_type: unsafe extern "C" fn(Object, *const c_char),
    response_headers: unsafe extern "C" fn(Object, *mut c_void),
    soup_headers_new: unsafe extern "C" fn(c_int) -> *mut c_void,
    soup_headers_append: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char),
    content_manager_new: unsafe extern "C" fn() -> Object,
    register_handler: unsafe extern "C" fn(Object, *const c_char, *const c_char) -> c_int,
    script_new: unsafe extern "C" fn(
        *const c_char,
        c_int,
        c_int,
        *const *const c_char,
        *const *const c_char,
    ) -> *mut c_void,
    add_script: unsafe extern "C" fn(Object, *mut c_void),
    script_unref: unsafe extern "C" fn(*mut c_void),
    load_uri: unsafe extern "C" fn(Object, *const c_char),
    uri_of_view: unsafe extern "C" fn(Object) -> *const c_char,
    evaluate: unsafe extern "C" fn(
        Object,
        *const c_char,
        isize,
        *const c_char,
        *const c_char,
        *mut c_void,
        *mut c_void,
        *mut c_void,
    ),
    set_background: unsafe extern "C" fn(Object, *const gtk::gdk::ffi::GdkRGBA),
    value_string: unsafe extern "C" fn(Object) -> *mut c_char,
    decision_action: unsafe extern "C" fn(Object) -> *mut c_void,
    action_request: unsafe extern "C" fn(*mut c_void) -> Object,
    request_get_uri: unsafe extern "C" fn(Object) -> *const c_char,
    decision_ignore: unsafe extern "C" fn(Object),
}

impl Api {
    fn load() -> Option<Self> {
        // SAFETY: names and signatures are those WebKitGTK 6.0, JavaScriptCore 6.0 and libsoup 3
        // document; the libraries stay loaded as long as the table does.
        unsafe {
            let open = |names: &[&str]| names.iter().find_map(|n| Library::new(n).ok());
            let webkit = open(&["libwebkitgtk-6.0.so.4", "libwebkitgtk-6.0.so"])?;
            let jsc = open(&["libjavascriptcoregtk-6.0.so.1", "libjavascriptcoregtk-6.0.so"])?;
            let soup = open(&["libsoup-3.0.so.0", "libsoup-3.0.so"])?;
            Some(Self {
                view_type: *webkit.get(b"webkit_web_view_get_type\0").ok()?,
                session_ephemeral: *webkit
                    .get(b"webkit_network_session_new_ephemeral\0")
                    .ok()?,
                context_new: *webkit.get(b"webkit_web_context_new\0").ok()?,
                register_scheme: *webkit
                    .get(b"webkit_web_context_register_uri_scheme\0")
                    .ok()?,
                request_uri: *webkit.get(b"webkit_uri_scheme_request_get_uri\0").ok()?,
                request_finish_with: *webkit
                    .get(b"webkit_uri_scheme_request_finish_with_response\0")
                    .ok()?,
                request_finish_error: *webkit
                    .get(b"webkit_uri_scheme_request_finish_error\0")
                    .ok()?,
                response_new: *webkit.get(b"webkit_uri_scheme_response_new\0").ok()?,
                response_status: *webkit
                    .get(b"webkit_uri_scheme_response_set_status\0")
                    .ok()?,
                response_type: *webkit
                    .get(b"webkit_uri_scheme_response_set_content_type\0")
                    .ok()?,
                response_headers: *webkit
                    .get(b"webkit_uri_scheme_response_set_http_headers\0")
                    .ok()?,
                soup_headers_new: *soup.get(b"soup_message_headers_new\0").ok()?,
                soup_headers_append: *soup.get(b"soup_message_headers_append\0").ok()?,
                content_manager_new: *webkit.get(b"webkit_user_content_manager_new\0").ok()?,
                register_handler: *webkit
                    .get(b"webkit_user_content_manager_register_script_message_handler\0")
                    .ok()?,
                script_new: *webkit.get(b"webkit_user_script_new\0").ok()?,
                add_script: *webkit.get(b"webkit_user_content_manager_add_script\0").ok()?,
                script_unref: *webkit.get(b"webkit_user_script_unref\0").ok()?,
                load_uri: *webkit.get(b"webkit_web_view_load_uri\0").ok()?,
                uri_of_view: *webkit.get(b"webkit_web_view_get_uri\0").ok()?,
                evaluate: *webkit.get(b"webkit_web_view_evaluate_javascript\0").ok()?,
                set_background: *webkit.get(b"webkit_web_view_set_background_color\0").ok()?,
                value_string: *jsc.get(b"jsc_value_to_string\0").ok()?,
                decision_action: *webkit
                    .get(b"webkit_navigation_policy_decision_get_navigation_action\0")
                    .ok()?,
                action_request: *webkit.get(b"webkit_navigation_action_get_request\0").ok()?,
                request_get_uri: *webkit.get(b"webkit_uri_request_get_uri\0").ok()?,
                decision_ignore: *webkit.get(b"webkit_policy_decision_ignore\0").ok()?,
                _webkit: webkit,
                _jsc: jsc,
                _soup: soup,
            })
        }
    }
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(Api::load).as_ref()
}

const SCHEME: &str = "kaeri-book";

/// What the scheme handler needs: the engine, and the way back to ask for a request's answer.
struct Handler {
    engine: std::sync::Weak<kaeri_engine::engine::Engine>,
}

/// Frees the scheme's `Handler` as its web context goes.
unsafe extern "C" fn drop_handler(data: *mut c_void) {
    // SAFETY: `data` is the Box<Handler> made at registration, freed once, here.
    drop(unsafe { Box::from_raw(data.cast::<Handler>()) });
}

/// Answers one request from the engine's served book.
unsafe extern "C" fn serve(request: Object, data: *mut c_void) {
    let Some(api) = api() else { return };
    // SAFETY: `data` is the Box<Handler> made at registration, alive for the context.
    let handler = unsafe { &*(data as *const Handler) };
    let uri =
        unsafe { CStr::from_ptr((api.request_uri)(request)) }.to_string_lossy().into_owned();
    let resource = handler.engine.upgrade().and_then(|engine| engine.book_resource(&uri));
    let Some(resource) = resource else {
        let mut error = glib::Error::new(gio::IOErrorEnum::NotFound, "not a book's file"); // i18n-skip
        unsafe { (api.request_finish_error)(request, error.to_glib_none_mut().0) };
        return;
    };
    let (stream, length): (gio::InputStream, i64) = match &resource.body {
        kaeri_model::reader::BookBody::Bytes { data } => {
            let len = data.len() as i64;
            (
                gio::MemoryInputStream::from_bytes(&glib::Bytes::from(data.as_slice()))
                    .upcast(),
                len,
            )
        }
        kaeri_model::reader::BookBody::File { path } => {
            let file = gio::File::for_path(path);
            let len = std::fs::metadata(path).map_or(-1, |m| m.len() as i64);
            match file.read(gio::Cancellable::NONE) {
                Ok(stream) => (stream.upcast(), len),
                Err(error) => {
                    let mut error = error;
                    unsafe { (api.request_finish_error)(request, error.to_glib_none_mut().0) };
                    return;
                }
            }
        }
    };
    unsafe {
        let response = (api.response_new)(stream.as_ptr().cast(), length);
        (api.response_status)(response, u32::from(resource.status), std::ptr::null());
        if let Ok(mime) = CString::new(resource.mime.as_str()) {
            (api.response_type)(response, mime.as_ptr());
        }
        // SOUP_MESSAGE_HEADERS_RESPONSE is 1; the response takes the headers.
        let headers = (api.soup_headers_new)(1);
        for header in &resource.headers {
            if let (Ok(name), Ok(value)) =
                (CString::new(header.name.as_str()), CString::new(header.value.as_str()))
            {
                (api.soup_headers_append)(headers, name.as_ptr(), value.as_ptr());
            }
        }
        (api.response_headers)(response, headers);
        (api.request_finish_with)(request, response);
        gobject_ffi::g_object_unref(response.cast());
    }
}

/// One novel open in a web view.
pub struct NovelView {
    pub widget: gtk::Widget,
    view: glib::Object,
    send: Rc<dyn Fn(ReaderCommand)>,
    session: u64,
    chapter: ChapterId,
    spine: Vec<(String, bool)>,
    tell: Rc<dyn Fn(NovelSays)>,
    /// How the document loading now came: a turn past a document's edge, or a link the
    /// person followed. Anything else loaded is placed.
    ready_how: Cell<Option<TurnHow>>,
}

/// What the novel tells the reader around it.
pub enum NovelSays {
    /// The page showing and the document's length in pages, for the scrubber.
    Page { page: u32, pages: u32 },
    /// A tap in the middle third: the chrome comes or goes.
    Middle,
    /// Past the last page of the last document.
    End,
    /// The page can't show the text any more: its web process stopped, or the document
    /// failed to load.
    Failed,
}

impl NovelView {
    pub fn new(
        ctx: &Rc<Ctx>,
        open: &NovelOpenView,
        send: Rc<dyn Fn(ReaderCommand)>,
        tell: Rc<dyn Fn(NovelSays)>,
    ) -> Result<Rc<Self>, String> {
        if !crate::trailer::webkit::available() {
            return Err("WebKitGTK 6 is not available".to_owned()); // i18n-skip
        }
        let api = api().ok_or_else(|| "WebKitGTK 6 could not be loaded".to_owned())?; // i18n-skip
        let assets = kaeri_engine::reading::novel::assets();
        let engine = ctx.hub.engine_link();
        // The header's room goes in before the paginator starts, which reads it with the page's
        // own setup: a configure sent with the page still loading finds no paginator yet.
        let setup = format!("window.kaeriSetup = {{top: {HEAD_ROOM}}};\n{}", assets.script); // i18n-skip
        let script = CString::new(setup).map_err(|e| e.to_string())?;
        // SAFETY: every returned GObject is taken full and released with its wrapper; strings
        // outlive their calls; the handler is the context's, freed with it.
        let view = unsafe {
            let session: glib::Object = from_glib_full((api.session_ephemeral)());
            let context: glib::Object = from_glib_full((api.context_new)());
            let handler = Box::into_raw(Box::new(Handler { engine }));
            let scheme = CString::new(SCHEME).map_err(|e| e.to_string())?;
            (api.register_scheme)(
                context.as_ptr(),
                scheme.as_ptr(),
                serve,
                handler.cast(),
                Some(drop_handler),
            );
            let content: glib::Object = from_glib_full((api.content_manager_new)());
            if (api.register_handler)(content.as_ptr(), c"kaeri".as_ptr(), std::ptr::null())
                == 0
            {
                return Err("the page could not be wired".to_owned()); // i18n-skip
            }
            // WEBKIT_USER_CONTENT_INJECT_ALL_FRAMES, WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START.
            let user_script =
                (api.script_new)(script.as_ptr(), 0, 0, std::ptr::null(), std::ptr::null());
            if !user_script.is_null() {
                (api.add_script)(content.as_ptr(), user_script);
                (api.script_unref)(user_script);
            }
            let view: glib::Object =
                glib::Object::builder_with_type(from_glib((api.view_type)()))
                    .property("network-session", &session)
                    .property("web-context", &context)
                    .property("user-content-manager", &content)
                    .build();
            let clear = gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.0);
            (api.set_background)(view.as_ptr(), clear.to_glib_none().0);
            (content, view)
        };
        let (content, view) = view;
        let widget =
            view.clone().downcast::<gtk::Widget>().map_err(|_| "no widget".to_owned())?;
        widget.set_hexpand(true);
        widget.set_vexpand(true);
        let this = Rc::new(Self {
            widget: widget.clone(),
            view: view.clone(),
            send,
            session: open.session,
            chapter: open.chapter.clone(),
            spine: open.spine.iter().map(|i| (i.url.clone(), i.linear)).collect(),
            tell,
            ready_how: Cell::new(None),
        });
        let read = api.value_string;
        content.connect_local("script-message-received::kaeri", false, {
            let this = Rc::downgrade(&this);
            let rtl =
                matches!(open.progression, kaeri_model::reader::ReadDirection::RightToLeft);
            move |values| {
                let value = values.get(1)?.get::<glib::Object>().ok()?;
                // SAFETY: the value is a live JSCValue for the call.
                let text = unsafe { read(value.as_ptr()) };
                if text.is_null() {
                    return None;
                }
                let message = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
                unsafe { glib::ffi::g_free(text.cast()) };
                if let Some(this) = this.upgrade() {
                    this.receive(&message, rtl);
                }
                None
            }
        });
        // Nothing leaves the book. The signal wants a gboolean on every path: true when the
        // handler decided (here, ignored the navigation), false to leave WebKit's default.
        view.connect_local("decide-policy", false, |values| {
            Some(decide_policy(values).to_value())
        });
        view.connect_local("context-menu", false, |_| Some(true.to_value()));
        // A web process that stops leaves a blank page that turns go to: the reader says the
        // chapter couldn't show, with Try again. Told after the signal, on the main loop.
        view.connect_local("web-process-terminated", false, {
            let tell = Rc::downgrade(&this.tell);
            move |_| {
                failed_later(&tell);
                None
            }
        });
        view.connect_local("load-failed", false, {
            let tell = Rc::downgrade(&this.tell);
            move |values| {
                if values.get(3).is_some_and(load_really_failed) {
                    failed_later(&tell);
                }
                Some(false.to_value())
            }
        });
        let mut url = open.entry.clone();
        if let Some(at) = &open.restore
            && let Some(item) = open.spine.get(at.spine as usize)
        {
            url = format!("{}#kaeri:pos={}", item.url, at.encode());
        }
        let uri = CString::new(url).map_err(|e| e.to_string())?;
        // SAFETY: the view is alive; the string is copied.
        unsafe { (api.load_uri)(view.as_ptr(), uri.as_ptr()) };
        if !open.style.is_empty() {
            this.style(&open.style);
        }
        Ok(this)
    }

    fn call(&self, script: &str) {
        let (Some(api), Ok(script)) = (api(), CString::new(script)) else { return };
        // SAFETY: the view is alive; the script is copied; no callback is given.
        unsafe {
            (api.evaluate)(
                self.view.as_ptr(),
                script.as_ptr(),
                -1,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
    }

    /// Hand the paginator the engine's style for the person's choices, with the first line
    /// kept clear of the header bar, as the reader board's Linux novel frame has it: the text
    /// starts below the header whether it shows or not.
    pub fn style(&self, style: &str) {
        if !style.is_empty() {
            let room = format!("{{top: {HEAD_ROOM}}}"); // i18n-skip
            self.call(&format!(
                "window.kaeriReader && window.kaeriReader.configure(Object.assign({style}, {room}))" // i18n-skip
            ));
        }
    }

    /// Show a page of the document, from the scrubber: 0 is the first.
    pub fn go_to(&self, page: u32) {
        self.call(&format!("window.kaeriReader && window.kaeriReader.goTo({page}, 'jump')")); // i18n-skip
    }

    /// Show the document's last page, from End.
    pub fn go_to_end(&self) {
        self.call(
            "window.kaeriReader && window.kaeriReader.goTo(window.kaeriReader.count() - 1, 'jump')", // i18n-skip
        );
    }

    /// Show a place the engine moved to: a resume, Back to page, another device's place. It
    /// reports as placed, so it neither offers Back to page nor takes it away.
    pub fn go_to_pos(&self, pos: &NovelPos) {
        let Some((url, _)) = self.spine.get(pos.spine as usize) else { return };
        let at = pos.encode();
        self.ready_how.set(None);
        if self.current() == Some(pos.spine as usize) {
            self.call(&format!(
                "window.kaeriReader && window.kaeriReader.goToPos('{at}', 'placed')" // i18n-skip
            ));
        } else {
            self.load(&format!("{url}#kaeri:pos={at}"));
        }
    }

    pub fn turn(&self, forward: bool) {
        self.call(&format!(
            "window.kaeriReader && window.kaeriReader.{}('key')", // i18n-skip
            if forward { "next" } else { "prev" }
        ));
    }

    fn receive(&self, text: &str, rtl: bool) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return };
        match json["type"].as_str() {
            Some("ready" | "page") => {
                let (Some(page), Some(pages)) = (json["page"].as_u64(), json["pages"].as_u64())
                else {
                    return;
                };
                let pos = &json["pos"];
                let place = NovelPos {
                    spine: pos["spine"].as_u64().unwrap_or(0) as u32,
                    path: pos["path"]
                        .as_array()
                        .map(|a| {
                            a.iter().filter_map(|v| v.as_u64()).map(|v| v as u32).collect()
                        })
                        .unwrap_or_default(),
                    offset: pos["offset"].as_u64().unwrap_or(0) as u32,
                };
                let how = match json["how"].as_str() {
                    Some("swipe") => TurnHow::Swipe,
                    Some("key") => TurnHow::Key,
                    Some("scroll") => TurnHow::Scroll,
                    Some("turn") => TurnHow::Turn,
                    Some("jump") => TurnHow::Jump,
                    // Ready after a load: how it came, or the app's own move.
                    _ => self.ready_how.get().unwrap_or(TurnHow::Placed),
                };
                if json["type"].as_str() == Some("ready") {
                    self.ready_how.set(None);
                }
                (self.send)(ReaderCommand::NovelAt {
                    session: self.session,
                    chapter: self.chapter.clone(),
                    pos: place,
                    page: page as u32,
                    pages: pages.max(1) as u32,
                    how,
                });
                (self.tell)(NovelSays::Page { page: page as u32, pages: pages.max(1) as u32 });
            }
            // Laid out again for new settings or a new size: the scrubber's length changes.
            Some("layout") => {
                if let (Some(page), Some(pages)) =
                    (json["page"].as_u64(), json["pages"].as_u64())
                {
                    (self.tell)(NovelSays::Page {
                        page: page as u32,
                        pages: pages.max(1) as u32,
                    });
                }
            }
            Some("key") => match json["key"].as_str() {
                Some("ArrowRight") => self.turn(!rtl), // i18n-skip
                Some("ArrowLeft") => self.turn(rtl),   // i18n-skip
                Some("ArrowDown" | "PageDown" | " ") => self.turn(true),
                Some("ArrowUp" | "PageUp") => self.turn(false),
                Some("Home") => self.go_to(0),   // i18n-skip
                Some("End") => self.go_to_end(), // i18n-skip
                _ => {}
            },
            Some("tap") => {
                if let Some(x) = json["x"].as_f64() {
                    if x < 0.33 {
                        self.turn(rtl);
                    } else if x > 0.67 {
                        self.turn(!rtl);
                    } else {
                        (self.tell)(NovelSays::Middle);
                    }
                }
            }
            // Past the last page of a document: the next one in the spine, or the end page.
            Some("end") => {
                let at = self.current();
                match at.and_then(|at| (at + 1..self.spine.len()).find(|&i| self.spine[i].1)) {
                    Some(next) => {
                        self.ready_how.set(Some(TurnHow::Turn));
                        self.load(&self.spine[next].0.clone());
                    }
                    None => (self.tell)(NovelSays::End),
                }
            }
            Some("start") => {
                let at = self.current();
                if let Some(prev) = at.and_then(|at| (0..at).rev().find(|&i| self.spine[i].1)) {
                    self.ready_how.set(Some(TurnHow::Turn));
                    self.load(&format!("{}#kaeri:end", self.spine[prev].0));
                }
            }
            Some("link") => {
                if json["internal"].as_bool() == Some(true)
                    && let Some(href) = json["href"].as_str()
                    && href.starts_with("kaeri-book://")
                {
                    self.ready_how.set(Some(TurnHow::Jump));
                    self.load(href);
                }
            }
            _ => {}
        }
    }

    fn current(&self) -> Option<usize> {
        let uri = self.uri()?;
        let path = uri.split('#').next()?;
        self.spine.iter().position(|(url, _)| url == path)
    }

    fn load(&self, url: &str) {
        let (Some(api), Ok(url)) = (api(), CString::new(url)) else { return };
        // SAFETY: the view is alive; the string is copied.
        unsafe { (api.load_uri)(self.view.as_ptr(), url.as_ptr()) };
    }

    fn uri(&self) -> Option<String> {
        let api = api()?;
        // SAFETY: the view is alive; the returned string is borrowed.
        let uri = unsafe { (api.uri_of_view)(self.view.as_ptr()) };
        (!uri.is_null()).then(|| unsafe { CStr::from_ptr(uri) }.to_string_lossy().into_owned())
    }
}

/// Tell the reader the page failed, once the signal that said so has returned.
fn failed_later(tell: &std::rc::Weak<dyn Fn(NovelSays)>) {
    let tell = tell.clone();
    glib::idle_add_local_once(move || {
        if let Some(tell) = tell.upgrade() {
            tell(NovelSays::Failed);
        }
    });
}

/// WebKitNetworkError's code for a load cancelled by the next one, as a quick second turn does.
const NETWORK_CANCELLED: i32 = 302;

/// Whether `load-failed`'s error is a real failure: not a load the next one replaced, and not
/// one the page's own policy stopped.
fn load_really_failed(error: &glib::Value) -> bool {
    let Ok(error) = error.get::<glib::Error>() else { return false };
    let code = {
        let raw: *const glib::ffi::GError = error.to_glib_none().0;
        // SAFETY: the error is alive for the call and owns its GError.
        unsafe { (*raw).code }
    };
    let domain = error.domain();
    let policy = domain == glib::Quark::from_str("WebKitPolicyError");
    let cancelled =
        domain == glib::Quark::from_str("WebKitNetworkError") && code == NETWORK_CANCELLED;
    !(policy || cancelled)
}

/// WebKitPolicyDecisionType's navigation and new-window actions, which both carry a URI.
const NAVIGATION_ACTION: i32 = 0;
const NEW_WINDOW_ACTION: i32 = 1;

/// The `decide-policy` handler: ignores a navigation that would leave the book and says it
/// decided; anything else is WebKit's to decide.
fn decide_policy(values: &[glib::Value]) -> bool {
    let Some(decision) = values.get(1).and_then(|v| v.get::<glib::Object>().ok()) else {
        return false;
    };
    // The decision type is a GEnum, which a typed get as i32 refuses.
    let kind = values.get(2).filter(|v| v.type_().is_a(glib::Type::ENUM)).map(|v| {
        // SAFETY: the value holds an enum, checked above.
        unsafe { gobject_ffi::g_value_get_enum(v.to_glib_none().0) }
    });
    let Some(kind) = kind else { return false };
    if !is_navigation(kind) {
        return false;
    }
    let Some(api) = self::api() else { return false };
    // SAFETY: the decision is alive for the signal; the action and request are borrowed.
    let uri = unsafe {
        let action = (api.decision_action)(decision.as_ptr());
        let request =
            if action.is_null() { std::ptr::null_mut() } else { (api.action_request)(action) };
        let uri =
            if request.is_null() { std::ptr::null() } else { (api.request_get_uri)(request) };
        (!uri.is_null()).then(|| CStr::from_ptr(uri).to_string_lossy().into_owned())
    };
    if stays_in_book(uri.as_deref()) {
        return false;
    }
    // SAFETY: the decision is alive for the signal.
    unsafe { (api.decision_ignore)(decision.as_ptr()) };
    true
}

fn is_navigation(kind: i32) -> bool {
    matches!(kind, NAVIGATION_ACTION | NEW_WINDOW_ACTION)
}

/// Whether a navigation stays inside the book: its own scheme or the blank page. A navigation
/// whose URI can't be read leaves.
fn stays_in_book(uri: Option<&str>) -> bool {
    uri.is_some_and(|uri| uri.starts_with("kaeri-book://") || uri == "about:blank")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_book_and_the_blank_page_stay() {
        assert!(stays_in_book(Some("kaeri-book://w/ch/3.xhtml#kaeri:pos=1")));
        assert!(stays_in_book(Some("about:blank")));
        assert!(!stays_in_book(Some("https://elsewhere.example/")));
        assert!(!stays_in_book(Some("file:///etc/passwd")));
        assert!(!stays_in_book(None));
    }

    #[test]
    fn navigations_and_new_windows_are_decided_responses_are_not() {
        assert!(is_navigation(NAVIGATION_ACTION));
        assert!(is_navigation(NEW_WINDOW_ACTION));
        assert!(!is_navigation(2));
    }

    #[test]
    fn decide_policy_answers_false_for_what_it_cannot_read() {
        // A missing decision, or a type that isn't an enum, leaves the default: never None.
        assert!(!decide_policy(&[]));
        let view = glib::Object::new::<glib::Object>();
        assert!(!decide_policy(&[view.to_value(), view.to_value(), 0i32.to_value()]));
    }
}
