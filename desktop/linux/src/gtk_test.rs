//! Tests that need GTK. GTK may only be used from the thread that started it, and the test
//! harness gives each test a thread of its own, so every GTK test hands its body to one thread
//! that starts GTK once and runs the bodies there, one at a time.

use std::{
    sync::{Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};

use gtk::glib;

type Job = Box<dyn FnOnce() + Send>;
type Done = mpsc::Sender<std::thread::Result<()>>;
type Queue = Mutex<mpsc::Sender<(Job, Done)>>;

/// The GTK thread's queue, or none when GTK cannot start (no display).
fn queue() -> Option<&'static Queue> {
    static QUEUE: OnceLock<Option<Queue>> = OnceLock::new();
    QUEUE
        .get_or_init(|| {
            let (jobs, queued) = mpsc::channel::<(Job, Done)>();
            let (started, ready) = mpsc::channel();
            std::thread::spawn(move || {
                poison_freed_memory();
                let ok = gtk::init().is_ok() && adw::init().is_ok();
                let _ = started.send(ok);
                if !ok {
                    return;
                }
                for (job, done) in queued {
                    let _ =
                        done.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)));
                }
            });
            ready.recv().unwrap_or(false).then(|| Mutex::new(jobs))
        })
        .as_ref()
}

/// Run `body` on the GTK thread and fail the calling test with its panic's message. Without a
/// display it says so and passes: CI runs these under xvfb-run, and a desktop has a display.
pub fn run(body: impl FnOnce() + Send + 'static) {
    let Some(queue) = queue() else {
        eprintln!("no display: this test needs one"); // i18n-skip
        return;
    };
    let (done, result) = mpsc::channel();
    queue
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .send((Box::new(body), done))
        .expect("the GTK thread runs");
    // Panicked again here rather than resumed, so the message is printed under this test: the
    // GTK thread's own output goes to whichever test started it.
    if let Err(panic) = result.recv().expect("the GTK thread answers") {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("the test panicked on the GTK thread"); // i18n-skip
        panic!("{message}");
    }
}

/// Run GTK's main loop for `for_ms`.
pub fn pump(for_ms: u64) {
    let until = Instant::now() + Duration::from_millis(for_ms);
    let main = glib::MainContext::default();
    while Instant::now() < until {
        main.iteration(false);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Fills memory with a pattern as it is freed, so a read of a freed object crashes every time
/// rather than now and then. Set before GTK starts.
fn poison_freed_memory() {
    #[cfg(target_env = "gnu")]
    {
        unsafe extern "C" {
            fn mallopt(param: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
        }
        const M_PERTURB: std::ffi::c_int = -6;
        // SAFETY: glibc's own tuning call, made before any GTK object exists.
        unsafe { mallopt(M_PERTURB, 0xa5) };
    }
}
