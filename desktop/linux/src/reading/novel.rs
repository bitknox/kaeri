//! A novel in WebKitGTK 6, loaded by name when the first novel opens.
//!
//! The engine serves the book: every request to `kaeri-book://` is answered by it, with the
//! policy headers it chose. The page is paginated by the engine's script, injected at the start
//! of every document, which talks back through the `kaeri` message handler with JSON lines. The
//! view can load nothing else: any navigation off the book scheme is refused, and the web view
//! runs in an ephemeral session.
//!
//! A main document the engine does not serve never replaces the page shown, and only the book's
//! own documents keep a place: see [`Cross`].
//!
//! WebKitGTK is not linked. Where it is missing, or its sandbox cannot start, `NovelView::new`
//! says so and the reader shows that the chapter cannot open here.

use std::{
    cell::RefCell,
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
    decision_response: unsafe extern "C" fn(Object) -> Object,
    decision_main_resource: unsafe extern "C" fn(Object) -> c_int,
    response_status_code: unsafe extern "C" fn(Object) -> u32,
    response_uri: unsafe extern "C" fn(Object) -> *const c_char,
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
                decision_response: *webkit
                    .get(b"webkit_response_policy_decision_get_response\0")
                    .ok()?,
                decision_main_resource: *webkit
                    .get(b"webkit_response_policy_decision_is_main_frame_main_resource\0")
                    .ok()?,
                response_status_code: *webkit
                    .get(b"webkit_uri_response_get_status_code\0")
                    .ok()?,
                response_uri: *webkit.get(b"webkit_uri_response_get_uri\0").ok()?,
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
        (api.response_status)(response, u32::from(status_of(&resource)), std::ptr::null());
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

/// The status a book's answer goes with: an empty body is no document to show, whatever the
/// engine said.
fn status_of(resource: &kaeri_model::reader::BookResource) -> u16 {
    match &resource.body {
        kaeri_model::reader::BookBody::Bytes { data } if data.is_empty() => 404,
        _ => resource.status,
    }
}

/// Whether a main document answering with `status` is refused: the navigation ends and the
/// page shown stays, where an error's empty answer would lay out as a blank page.
fn refuses(status: u32) -> bool {
    !(200..300).contains(&status)
}

/// One novel open in a web view.
pub struct NovelView {
    pub widget: gtk::Widget,
    view: glib::Object,
    send: Rc<dyn Fn(ReaderCommand)>,
    session: u64,
    chapter: ChapterId,
    cross: RefCell<Cross>,
    tell: Rc<dyn Fn(NovelSays)>,
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
            cross: RefCell::new(Cross::new(
                open.spine.iter().map(|i| (i.url.clone(), i.linear)).collect(),
                &open.entry,
                open.restore.as_ref(),
            )),
            tell,
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
        // Nothing leaves the book, and a main document the engine did not serve never replaces
        // the page shown. The signal wants a gboolean on every path: true when the handler
        // decided (here, ignored the navigation or the response), false to leave WebKit's default.
        view.connect_local("decide-policy", false, {
            let this = Rc::downgrade(&this);
            move |values| {
                let refused = |uri: String| {
                    let this = this.clone();
                    glib::idle_add_local_once(move || {
                        if let Some(this) = this.upgrade() {
                            this.refused(&uri);
                        }
                    });
                };
                Some(decide_policy(values, &refused).to_value())
            }
        });
        // A load that failed leaves the page as it was, never WebKit's error page, which would
        // lay out at the book's address as if it were the book. A real failure, not a load the
        // next one replaced or one a policy stopped, has the reader say the chapter couldn't
        // show, with Try again.
        view.connect_local("load-failed", false, {
            let tell = Rc::downgrade(&this.tell);
            let this = Rc::downgrade(&this);
            move |values| {
                if values.get(3).is_some_and(load_really_failed) {
                    failed_later(&tell);
                    return Some(true.to_value());
                }
                let uri = values.get(2).and_then(|v| v.get::<Option<String>>().ok()).flatten();
                if let Some(uri) = uri {
                    let this = this.clone();
                    glib::idle_add_local_once(move || {
                        if let Some(this) = this.upgrade() {
                            this.refused(&uri);
                        }
                    });
                }
                Some(true.to_value())
            }
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
        let opening = this.cross.borrow_mut().open();
        this.run(opening);
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
        let at = pos.encode();
        let act = {
            let mut cross = self.cross.borrow_mut();
            let Some((url, _)) = cross.spine.get(pos.spine as usize).cloned() else { return };
            cross.ready_how = None;
            let shown = cross.shown.clone();
            if cross.index(shown.as_deref()) == Some(pos.spine as usize) {
                None
            } else {
                Some(cross.load(format!("{url}#kaeri:pos={at}"))) // i18n-skip
            }
        };
        match act {
            Some(act) => self.run(vec![act]),
            None => self.call(&format!(
                "window.kaeriReader && window.kaeriReader.goToPos('{at}', 'placed')" // i18n-skip
            )),
        }
    }

    pub fn turn(&self, forward: bool) {
        self.call(&format!(
            "window.kaeriReader && window.kaeriReader.{}('key')", // i18n-skip
            if forward { "next" } else { "prev" }
        ));
    }

    fn receive(&self, text: &str, rtl: bool) {
        let uri = self.uri();
        let acts = self.cross.borrow_mut().receive(text, uri.as_deref(), rtl);
        self.run(acts);
    }

    /// The main document at `uri` did not come: the page shown stays.
    fn refused(&self, uri: &str) {
        let acts = self.cross.borrow_mut().refused(uri);
        self.run(acts);
    }

    fn run(&self, acts: Vec<Act>) {
        for act in acts {
            match act {
                Act::Place { pos, page, pages, how } => {
                    (self.send)(ReaderCommand::NovelAt {
                        session: self.session,
                        chapter: self.chapter.clone(),
                        pos,
                        page,
                        pages,
                        how,
                    });
                }
                Act::Pages { page, pages } => (self.tell)(NovelSays::Page { page, pages }),
                Act::Load(url) => self.load(&url),
                Act::Turn(forward) => self.turn(forward),
                Act::Middle => (self.tell)(NovelSays::Middle),
                Act::First => self.go_to(0),
                Act::Last => self.go_to_end(),
                Act::End => {
                    let last = self.cross.borrow().reported.clone();
                    if let Some(end) = end_at(self.session, &self.chapter, last.as_ref()) {
                        (self.send)(end);
                    }
                    (self.tell)(NovelSays::End);
                }
            }
        }
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

/// What the view does for a message from the page or a load that failed.
#[derive(Debug, Clone, PartialEq)]
enum Act {
    /// Keep this place: a page of the book's own document.
    Place {
        pos: NovelPos,
        page: u32,
        pages: u32,
        how: TurnHow,
    },
    /// The page showing and the document's length in pages, for the scrubber.
    Pages {
        page: u32,
        pages: u32,
    },
    Load(String),
    Turn(bool),
    Middle,
    /// The document's first page, from Home.
    First,
    /// The document's last page, from End.
    Last,
    /// Past the last page of the last document.
    End,
}

/// What a page the paginator laid out is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shown {
    /// A document of the reading order, at the address shown: its place is kept.
    Book,
    /// A document of the book outside the reading order: it shows and keeps no place.
    Outside,
    /// Not the book's document at the address shown: it keeps no place.
    NotServed,
}

/// Where a novel's web view goes between the book's documents, and which of its pages keep a
/// place. Only the book's own documents keep one: a document of the book outside the reading
/// order (its contents page, a note) shows and keeps none, and a page that is not the book's,
/// such as an error's empty answer, keeps none and goes back once to the last place shown.
struct Cross {
    spine: Vec<(String, bool)>,
    /// The start every address of the open session's book has.
    root: String,
    /// Where the chapter opened: what a page with no place shown yet goes back to.
    opening: String,
    /// The last place the book showed.
    last: Option<(usize, NovelPos)>,
    /// Going back to a known place after a page that was not the book's, until the book shows.
    recovering: bool,
    /// The address of the book's document shown, or `None` while none is.
    shown: Option<String>,
    /// The address being loaded, until its page is laid out or its load fails.
    pending: Option<String>,
    /// How the document loading now came: a turn past a document's edge, or a link the
    /// person followed. Anything else loaded is placed.
    ready_how: Option<TurnHow>,
    /// The last place the book's page reported, with its document's page count: the end page
    /// is reported from it.
    reported: Option<(NovelPos, u32)>,
}

impl Cross {
    fn new(spine: Vec<(String, bool)>, entry: &str, restore: Option<&NovelPos>) -> Self {
        let mut opening = entry.to_owned();
        if let Some(at) = restore
            && let Some((url, _)) = spine.get(at.spine as usize)
        {
            opening = format!("{url}#kaeri:pos={}", at.encode()); // i18n-skip
        }
        // `kaeri-book://<session>/`: the scheme, the session and the slash after it.
        let root = match entry.split_once("://") {
            Some((scheme, rest)) => {
                format!("{scheme}://{}/", rest.split('/').next().unwrap_or_default())
            }
            None => entry.to_owned(),
        };
        Self {
            spine,
            root,
            opening,
            last: None,
            recovering: false,
            shown: None,
            pending: None,
            ready_how: None,
            reported: None,
        }
    }

    /// The chapter's first load.
    fn open(&mut self) -> Vec<Act> {
        vec![self.load(self.opening.clone())]
    }

    fn load(&mut self, url: String) -> Act {
        self.pending = Some(document(&url));
        Act::Load(url)
    }

    /// The spine document at `address`, whatever its fragment and however its escapes are
    /// written.
    fn index(&self, address: Option<&str>) -> Option<usize> {
        let path = document(address?);
        self.spine.iter().position(|(url, _)| document(url) == path)
    }

    /// Whether `address` names a file of the open book.
    fn in_book(&self, address: Option<&str>) -> bool {
        address.is_some_and(|a| document(a).starts_with(&document(&self.root)))
    }

    fn shown(&mut self, address: Option<&str>, spine: u32, pos: &NovelPos) -> Shown {
        match self.index(address) {
            Some(index) if index == spine as usize => {
                self.last = Some((index, pos.clone()));
                self.recovering = false;
                Shown::Book
            }
            None if self.in_book(address) => {
                self.recovering = false;
                Shown::Outside
            }
            _ => Shown::NotServed,
        }
    }

    /// A turn on the page shown: whether its place is the book's to keep.
    fn keeps(&mut self, spine: u32, pos: &NovelPos) -> bool {
        match self.index(self.shown.as_deref()) {
            Some(index) if index == spine as usize => {
                self.last = Some((index, pos.clone()));
                true
            }
            _ => false,
        }
    }

    /// The last place the book showed, or where the chapter opened.
    fn back(&self) -> String {
        self.last
            .as_ref()
            .and_then(|(index, pos)| {
                self.spine
                    .get(*index)
                    .map(|(url, _)| format!("{url}#kaeri:pos={}", pos.encode())) // i18n-skip
            })
            .unwrap_or_else(|| self.opening.clone())
    }

    /// Back to the last place, once until the book shows again.
    fn recover(&mut self) -> Vec<Act> {
        if self.recovering {
            return Vec::new();
        }
        self.recovering = true;
        self.ready_how = None;
        let back = self.back();
        vec![self.load(back)]
    }

    /// Past the end (`by` 1) or before the start (`by` -1) of the document shown: the next
    /// linear document that way, from the last place when the page is outside the reading order.
    fn step(&mut self, by: isize) -> Vec<Act> {
        let shown = self.shown.clone();
        let Some(current) = self.index(shown.as_deref()).or(self.last.as_ref().map(|l| l.0))
        else {
            self.ready_how = None;
            let back = self.back();
            return vec![self.load(back)];
        };
        let mut i = current as isize + by;
        while let Some((url, linear)) = usize::try_from(i).ok().and_then(|i| self.spine.get(i))
        {
            if *linear {
                let url = if by < 0 { format!("{url}#kaeri:end") } else { url.clone() }; // i18n-skip
                self.ready_how = Some(TurnHow::Turn);
                return vec![self.load(url)];
            }
            i += by;
        }
        if by > 0 { vec![Act::End] } else { Vec::new() }
    }

    /// The main document at `uri` did not come: the page shown stays with its keys, and with
    /// none of the book shown the chapter goes back to its last place, once.
    fn refused(&mut self, uri: &str) -> Vec<Act> {
        if self.pending.as_deref() != Some(document(uri).as_str()) {
            return Vec::new();
        }
        self.pending = None;
        let shown = self.shown.clone();
        if self.index(shown.as_deref()).is_some() || self.in_book(shown.as_deref()) {
            Vec::new()
        } else {
            self.recover()
        }
    }

    /// A message from the paginator, with the web view's address when it came.
    fn receive(&mut self, text: &str, uri: Option<&str>, rtl: bool) -> Vec<Act> {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
            return Vec::new();
        };
        match json["type"].as_str() {
            Some(kind @ ("ready" | "page")) => {
                let (Some(page), Some(pages)) = (json["page"].as_u64(), json["pages"].as_u64())
                else {
                    return Vec::new();
                };
                let (page, pages) = (page as u32, pages.max(1) as u32);
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
                    _ => self.ready_how.unwrap_or(TurnHow::Placed),
                };
                let spine = json["spine"].as_u64().unwrap_or(0) as u32;
                let keep = if kind == "ready" {
                    self.pending = None;
                    self.ready_how = None;
                    match self.shown(uri, spine, &place) {
                        Shown::Book => {
                            self.shown = uri.map(str::to_owned);
                            true
                        }
                        Shown::Outside => {
                            self.shown = uri.map(str::to_owned);
                            false
                        }
                        Shown::NotServed => {
                            self.shown = None;
                            return self.recover();
                        }
                    }
                } else {
                    self.keeps(spine, &place)
                };
                let mut acts = Vec::new();
                if keep {
                    self.reported = Some((place.clone(), pages));
                    acts.push(Act::Place { pos: place, page, pages, how });
                }
                acts.push(Act::Pages { page, pages });
                acts
            }
            // Laid out again for new settings or a new size: the scrubber's length changes.
            Some("layout") => match (json["page"].as_u64(), json["pages"].as_u64()) {
                (Some(page), Some(pages)) => {
                    vec![Act::Pages { page: page as u32, pages: pages.max(1) as u32 }]
                }
                _ => Vec::new(),
            },
            Some("key") => match json["key"].as_str() {
                Some("ArrowRight") => vec![Act::Turn(!rtl)], // i18n-skip
                Some("ArrowLeft") => vec![Act::Turn(rtl)],   // i18n-skip
                Some("ArrowDown" | "PageDown" | " ") => vec![Act::Turn(true)],
                Some("ArrowUp" | "PageUp") => vec![Act::Turn(false)],
                Some("Home") => vec![Act::First], // i18n-skip
                Some("End") => vec![Act::Last],   // i18n-skip
                _ => Vec::new(),
            },
            Some("tap") => match json["x"].as_f64() {
                Some(x) if x < 0.33 => vec![Act::Turn(rtl)],
                Some(x) if x > 0.67 => vec![Act::Turn(!rtl)],
                Some(_) => vec![Act::Middle],
                None => Vec::new(),
            },
            // Past the last page of a document: the next one in the spine, or the end page.
            Some("end") => self.step(1),
            Some("start") => self.step(-1),
            // Links to any file of the open book are followed; anything else stays.
            Some("link") => match json["href"].as_str() {
                Some(href)
                    if json["internal"].as_bool() == Some(true) && self.in_book(Some(href)) =>
                {
                    self.ready_how = Some(TurnHow::Jump);
                    vec![self.load(href.to_owned())]
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

/// The document an address names: without its fragment and with its escapes read.
fn document(address: &str) -> String {
    let path = address.split('#').next().unwrap_or_default();
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
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

/// WebKitPolicyDecisionType's navigation and new-window actions, which both carry a URI, and
/// its response.
const NAVIGATION_ACTION: i32 = 0;
const NEW_WINDOW_ACTION: i32 = 1;
const RESPONSE: i32 = 2;

/// The `decide-policy` handler: ignores a navigation that would leave the book, and a main
/// document that answers with an error (telling `refused` its URI), and says it decided;
/// anything else is WebKit's to decide.
fn decide_policy(values: &[glib::Value], refused: &dyn Fn(String)) -> bool {
    let Some(decision) = values.get(1).and_then(|v| v.get::<glib::Object>().ok()) else {
        return false;
    };
    // The decision type is a GEnum, which a typed get as i32 refuses.
    let kind = values.get(2).filter(|v| v.type_().is_a(glib::Type::ENUM)).map(|v| {
        // SAFETY: the value holds an enum, checked above.
        unsafe { gobject_ffi::g_value_get_enum(v.to_glib_none().0) }
    });
    let Some(kind) = kind else { return false };
    if kind == RESPONSE {
        return decide_response(&decision, refused);
    }
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

/// A response: the main document's error answer is ignored, which ends its navigation with the
/// page shown as it was.
fn decide_response(decision: &glib::Object, refused: &dyn Fn(String)) -> bool {
    let Some(api) = self::api() else { return false };
    // SAFETY: the decision is alive for the signal; the response and its URI are borrowed.
    let (main, status, uri) = unsafe {
        let main = (api.decision_main_resource)(decision.as_ptr()) != 0;
        let response = (api.decision_response)(decision.as_ptr());
        if response.is_null() {
            return false;
        }
        let uri = (api.response_uri)(response);
        let uri = (!uri.is_null()).then(|| CStr::from_ptr(uri).to_string_lossy().into_owned());
        (main, (api.response_status_code)(response), uri)
    };
    if !main || !refuses(status) {
        return false;
    }
    // SAFETY: the decision is alive for the signal.
    unsafe { (api.decision_ignore)(decision.as_ptr()) };
    if let Some(uri) = uri {
        refused(uri);
    }
    true
}

fn is_navigation(kind: i32) -> bool {
    matches!(kind, NAVIGATION_ACTION | NEW_WINDOW_ACTION)
}

/// The end page as the engine hears it: the last place the page reported, at the page after
/// its last. The engine reads the chapter at once there, as "After the last page" asks. None
/// before the page has reported a place.
fn end_at(
    session: u64,
    chapter: &ChapterId,
    last: Option<&(NovelPos, u32)>,
) -> Option<ReaderCommand> {
    let (pos, pages) = last?;
    Some(ReaderCommand::NovelAt {
        session,
        chapter: chapter.clone(),
        pos: pos.clone(),
        page: *pages,
        pages: *pages,
        how: TurnHow::Turn,
    })
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
    fn the_end_page_is_the_page_after_the_last() {
        let chapter = ChapterId::new("7");
        assert!(end_at(3, &chapter, None).is_none(), "nothing reported yet");
        let pos = NovelPos { spine: 4, path: vec![2], offset: 0 };
        let Some(ReaderCommand::NovelAt { session, chapter: at, pos: place, page, pages, how }) =
            end_at(3, &chapter, Some(&(pos.clone(), 12)))
        else {
            panic!("the end page is a place");
        };
        assert_eq!((session, at, place), (3, chapter, pos));
        // The engine reads the chapter at once at page == pages.
        assert_eq!((page, pages, how), (12, 12, TurnHow::Turn));
    }

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
        assert!(!is_navigation(RESPONSE));
    }

    /// "Paper Lantern Vol 1" as the engine serves it for one session: three parts in the reading
    /// order with spaces (and a comma) in their file names, a contents page and a notes document
    /// in the manifest but outside the spine. Each part carries its spine index in its settings;
    /// a document outside the spine says the first. A part listed as missing, a file the book does
    /// not have and another session's address answer 404 with an empty body.
    struct Book {
        missing: Vec<&'static str>,
        /// Parts served without the reader's settings, which the paginator reads as its defaults.
        bare: Vec<&'static str>,
    }

    const PARTS: [&str; 3] = [
        "Text/Part 1 of Paper Lantern.xhtml",
        "Text/Part 2 of Paper Lantern.xhtml",
        "Text/Part 3, Harbour Lights.xhtml",
    ];
    const OUTSIDE: [&str; 2] = ["nav.xhtml", "Text/Notes for Part 1.xhtml"];

    impl Book {
        /// The address the engine writes: spaces escaped, the comma kept as it is.
        fn url(path: &str) -> String {
            format!("kaeri-book://7/{}", path.replace(' ', "%20"))
        }

        fn spine() -> Vec<(String, bool)> {
            PARTS.iter().map(|p| (Self::url(p), true)).collect()
        }

        fn resource(&self, address: &str) -> kaeri_model::reader::BookResource {
            use kaeri_model::reader::{BookBody, BookResource};
            let path = document(address);
            let name = path.strip_prefix("kaeri-book://7/").unwrap_or_default();
            let known = PARTS.contains(&name) || OUTSIDE.contains(&name);
            if !known || self.missing.contains(&name) {
                let body = BookBody::Bytes { data: Vec::new() };
                return BookResource {
                    status: 404,
                    mime: "text/plain".into(),
                    headers: Vec::new(),
                    body,
                };
            }
            let spine = PARTS.iter().position(|p| *p == name).unwrap_or(0);
            let setup = if self.bare.contains(&name) {
                String::new()
            } else {
                format!(
                    "<meta name=\"kaeri-setup\" content=\"{{&quot;spine&quot;:{spine}}}\"/>"
                )
            };
            let text = format!(
                "<html><head>{setup}</head><body><p>The ferry keeper counted the lanterns.</p></body></html>"
            );
            let body = BookBody::Bytes { data: text.into_bytes() };
            BookResource {
                status: 200,
                mime: "application/xhtml+xml".into(),
                headers: Vec::new(),
                body,
            }
        }
    }

    /// The web view's part, played here: each load is answered by the book through the view's
    /// own status and refusal; a refused document leaves the page as it was, and anything else
    /// is laid out, the paginator on an empty answer reading its defaults: spine 0, one page.
    struct Web {
        book: Book,
        cross: Cross,
        uri: Option<String>,
        loads: Vec<String>,
        places: Vec<(u32, u32, u32)>,
        ended: bool,
    }

    impl Web {
        fn open(book: Book) -> Self {
            let cross = Cross::new(Book::spine(), &Book::url(PARTS[0]), None);
            let mut web = Self {
                book,
                cross,
                uri: None,
                loads: Vec::new(),
                places: Vec::new(),
                ended: false,
            };
            let acts = web.cross.open();
            web.run(acts);
            web
        }

        fn run(&mut self, acts: Vec<Act>) {
            for act in acts {
                match act {
                    Act::Place { pos, page, pages, .. } => {
                        self.places.push((pos.spine, page, pages))
                    }
                    Act::Load(url) => self.navigate(url),
                    Act::End => self.ended = true,
                    _ => {}
                }
            }
        }

        fn navigate(&mut self, url: String) {
            self.loads.push(url.clone());
            assert!(self.loads.len() < 20, "loads without end: {:?}", self.loads);
            let resource = self.book.resource(&url);
            if refuses(u32::from(status_of(&resource))) {
                let acts = self.cross.refused(&url);
                return self.run(acts);
            }
            self.uri = Some(url.clone());
            let kaeri_model::reader::BookBody::Bytes { data } = &resource.body else {
                unreachable!()
            };
            let body = String::from_utf8_lossy(data);
            let spine: u32 = body
                .split("&quot;spine&quot;:")
                .nth(1)
                .and_then(|r| r.split('}').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let pages = if body.is_empty() { 1 } else { 12 };
            let page = if url.ends_with("#kaeri:end") { pages - 1 } else { 0 };
            self.say(&format!(
                r#"{{"type":"ready","spine":{spine},"page":{page},"pages":{pages},"pos":{{"spine":{spine},"path":[{page}],"offset":0}}}}"#
            ));
        }

        fn say(&mut self, json: &str) {
            let acts = self.cross.receive(json, self.uri.clone().as_deref(), false);
            self.run(acts);
        }

        /// A turn inside the document shown, to `page`.
        fn turn(&mut self, spine: u32, page: u32) {
            self.say(&format!(
                r#"{{"type":"page","spine":{spine},"page":{page},"pages":12,"pos":{{"spine":{spine},"path":[{page}],"offset":7}},"how":"key"}}"#
            ));
        }

        fn link(&mut self, href: &str, internal: bool) {
            self.say(&format!(r#"{{"type":"link","href":"{href}","internal":{internal}}}"#));
        }
    }

    /// Part 2 does not come: part 1 stays with its keys, no place is kept from the empty
    /// answer, and the next key asks for part 2 again rather than a part further on.
    #[test]
    fn a_part_that_does_not_come_leaves_the_page_and_keeps_no_place() {
        let mut web = Web::open(Book { missing: vec![PARTS[1]], bare: Vec::new() });
        web.turn(0, 11);
        let before = web.places.len();
        web.say(r#"{"type":"end","spine":0}"#);
        assert_eq!(web.loads.last(), Some(&Book::url(PARTS[1])), "part 2 is asked for");
        assert_eq!(web.uri, Some(Book::url(PARTS[0])), "part 1 stays");
        let after = &web.places[before..];
        assert!(
            after.iter().all(|p| p.0 == 0 && p.2 > 1),
            "no place from the empty answer: {after:?}"
        );
        web.say(r#"{"type":"end","spine":0}"#);
        assert_eq!(
            web.loads.last(),
            Some(&Book::url(PARTS[1])),
            "the next key asks for part 2 again"
        );
        assert!(!web.ended, "the end page stays away");
    }

    /// A page that is not the book's at its address keeps no place and goes back to the last
    /// place shown, once.
    #[test]
    fn a_page_that_is_not_the_books_keeps_no_place_and_goes_back_once() {
        let mut web = Web::open(Book { missing: Vec::new(), bare: vec![PARTS[1]] });
        web.turn(0, 11);
        let before = web.places.len();
        web.book.missing.push(PARTS[0]);
        web.say(r#"{"type":"end","spine":0}"#);
        assert!(
            web.places[before..].is_empty(),
            "no place from part 2's page without the book's settings"
        );
        assert_eq!(
            &web.loads[1..],
            &[Book::url(PARTS[1]), format!("{}#kaeri:pos=0/11/7", Book::url(PARTS[0]))],
            "back to part 1's last place, once, and no further when that does not come either"
        );
    }

    /// Links to the book's documents outside the reading order are followed and keep no place;
    /// a turn from them goes on from the last place shown, never to the end page. Escapes written
    /// another way still name their part, and links out of the book or to a file it does not
    /// have leave the page.
    #[test]
    fn documents_outside_the_spine_and_escaped_names_cross_and_keep_no_place() {
        let mut web = Web::open(Book { missing: Vec::new(), bare: Vec::new() });
        web.turn(0, 5);

        // A note outside the spine: shown, no place kept.
        let note = format!("{}#note-2", Book::url(OUTSIDE[1]));
        web.link(&note, true);
        assert_eq!(web.loads.last(), Some(&note), "the note is followed");
        assert_eq!(web.places.last(), Some(&(0, 5, 12)), "no place from the note");

        // Past the note's end: part 2, from the last place shown in part 1.
        web.say(r#"{"type":"end","spine":0}"#);
        assert!(!web.ended, "no end page from a note");
        assert_eq!(web.loads.last(), Some(&Book::url(PARTS[1])));
        assert_eq!(web.places.last().map(|p| p.0), Some(1));

        // The contents page outside the spine, then part 3 by a link that escapes its comma.
        web.link(&Book::url(OUTSIDE[0]), true);
        assert_eq!(web.loads.last(), Some(&Book::url(OUTSIDE[0])));
        assert_eq!(web.places.last().map(|p| p.0), Some(1), "no place from the contents page");
        let third = "kaeri-book://7/Text/Part%203%2C%20Harbour%20Lights.xhtml";
        web.link(third, true);
        assert_eq!(web.loads.last().map(String::as_str), Some(third));
        assert_eq!(web.places.last().map(|p| p.0), Some(2), "part 3 keeps its place");

        // Back from part 3's first page: part 2's last page.
        web.say(r#"{"type":"start","spine":2}"#);
        assert_eq!(web.loads.last(), Some(&format!("{}#kaeri:end", Book::url(PARTS[1]))));
        assert_eq!(web.places.last(), Some(&(1, 11, 12)));

        // Out of the book, and a file the book does not have: the page stays.
        let loads = web.loads.len();
        web.link("https://elsewhere.example/", false);
        web.link("kaeri-book://8/Text/Part%202%20of%20Paper%20Lantern.xhtml", true);
        assert_eq!(web.loads.len(), loads, "nothing loads out of the book or its session");
        web.link(&Book::url("Text/Part 9.xhtml"), true);
        assert_eq!(
            web.uri,
            Some(format!("{}#kaeri:end", Book::url(PARTS[1]))),
            "a missing file is refused"
        );
        assert_eq!(web.places.last(), Some(&(1, 11, 12)), "and keeps no place");
        web.say(r#"{"type":"end","spine":1}"#);
        assert_eq!(web.loads.last(), Some(&Book::url(PARTS[2])), "the keys go on from part 2");
    }

    #[test]
    fn an_empty_answer_and_an_error_are_refused() {
        use kaeri_model::reader::{BookBody, BookResource};
        let empty = BookResource {
            status: 200,
            mime: "application/xhtml+xml".into(),
            headers: Vec::new(),
            body: BookBody::Bytes { data: Vec::new() },
        };
        assert!(refuses(u32::from(status_of(&empty))));
        assert!(refuses(404));
        assert!(!refuses(200));
    }

    #[test]
    fn decide_policy_answers_false_for_what_it_cannot_read() {
        // A missing decision, or a type that isn't an enum, leaves the default: never None.
        let refused = |_: String| panic!("nothing was refused");
        assert!(!decide_policy(&[], &refused));
        let view = glib::Object::new::<glib::Object>();
        assert!(!decide_policy(&[view.to_value(), view.to_value(), 0i32.to_value()], &refused));
    }
}
