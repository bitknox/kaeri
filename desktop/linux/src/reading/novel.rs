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

/// Answers one request from the engine's served book.
unsafe extern "C" fn serve(request: Object, data: *mut c_void) {
    let Some(api) = api() else { return };
    // SAFETY: `data` is the Box<Handler> leaked at registration, alive for the context.
    let handler = unsafe { &*(data as *const Handler) };
    let uri =
        unsafe { CStr::from_ptr((api.request_uri)(request)) }.to_string_lossy().into_owned();
    let resource = handler.engine.upgrade().and_then(|engine| engine.book_resource(&uri));
    let Some(resource) = resource else {
        let mut error = glib::Error::new(gio::IOErrorEnum::NotFound, "not a book's file");
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
    on_end: Rc<dyn Fn()>,
}

impl NovelView {
    pub fn new(
        ctx: &Rc<Ctx>,
        open: &NovelOpenView,
        send: Rc<dyn Fn(ReaderCommand)>,
        on_end: Rc<dyn Fn()>,
    ) -> Result<Rc<Self>, String> {
        if !crate::trailer::webkit::available() {
            return Err("WebKitGTK 6 is not available".to_owned());
        }
        let api = api().ok_or_else(|| "WebKitGTK 6 could not be loaded".to_owned())?;
        let assets = kaeri_engine::reading::novel::assets();
        let engine = ctx.hub.engine_link();
        let script = CString::new(assets.script).map_err(|e| e.to_string())?;
        // SAFETY: every returned GObject is taken full and released with its wrapper; strings
        // outlive their calls; the leaked handler lives as long as the process.
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
                None,
            );
            let content: glib::Object = from_glib_full((api.content_manager_new)());
            if (api.register_handler)(content.as_ptr(), c"kaeri".as_ptr(), std::ptr::null())
                == 0
            {
                return Err("the page could not be wired".to_owned());
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
            on_end,
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
        // Nothing leaves the book.
        view.connect_local("decide-policy", false, move |values| {
            let decision = values.get(1)?.get::<glib::Object>().ok()?;
            let kind = values.get(2)?.get::<i32>().ok()?;
            // WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION is 0.
            if kind != 0 {
                return Some(false.to_value());
            }
            let api = self::api()?;
            // SAFETY: the decision is alive for the signal; the action and request are borrowed.
            let uri = unsafe {
                let action = (api.decision_action)(decision.as_ptr());
                let request = (api.action_request)(action);
                let uri = (api.request_get_uri)(request);
                if uri.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(uri).to_string_lossy().into_owned()
                }
            };
            if !uri.starts_with("kaeri-book://") && uri != "about:blank" {
                unsafe { (api.decision_ignore)(decision.as_ptr()) };
                return Some(true.to_value());
            }
            Some(false.to_value())
        });
        view.connect_local("context-menu", false, |_| Some(true.to_value()));
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
            this.style(&open.style, &open.count_key, &open.counted);
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

    /// Hand the paginator the engine's style for the person's choices.
    pub fn style(&self, style: &str, _count_key: &str, _counted: &[u32]) {
        if !style.is_empty() {
            self.call(&format!("window.kaeriReader && window.kaeriReader.configure({style})"));
        }
    }

    pub fn turn(&self, forward: bool) {
        self.call(&format!(
            "window.kaeriReader && window.kaeriReader.{}('key')",
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
                    _ => TurnHow::Jump,
                };
                (self.send)(ReaderCommand::NovelAt {
                    session: self.session,
                    chapter: self.chapter.clone(),
                    pos: place,
                    page: page as u32,
                    pages: pages.max(1) as u32,
                    how,
                });
            }
            Some("key") => match json["key"].as_str() {
                Some("ArrowRight") => self.turn(!rtl),
                Some("ArrowLeft") => self.turn(rtl),
                Some("ArrowDown" | "PageDown" | " ") => self.turn(true),
                Some("ArrowUp" | "PageUp") => self.turn(false),
                _ => {}
            },
            Some("tap") => {
                if let Some(x) = json["x"].as_f64() {
                    if x < 0.33 {
                        self.turn(rtl);
                    } else if x > 0.67 {
                        self.turn(!rtl);
                    }
                }
            }
            // Past the last page of a document: the next one in the spine, or the end page.
            Some("end") => {
                let at = self.current();
                match at.and_then(|at| (at + 1..self.spine.len()).find(|&i| self.spine[i].1)) {
                    Some(next) => self.load(&self.spine[next].0.clone()),
                    None => (self.on_end)(),
                }
            }
            Some("start") => {
                let at = self.current();
                if let Some(prev) = at.and_then(|at| (0..at).rev().find(|&i| self.spine[i].1)) {
                    self.load(&format!("{}#kaeri:end", self.spine[prev].0));
                }
            }
            Some("link") => {
                if json["internal"].as_bool() == Some(true)
                    && let Some(href) = json["href"].as_str()
                    && href.starts_with("kaeri-book://")
                {
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
