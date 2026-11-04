//! The device's own lock before the recovery key shows again: the account password, asked
//! through polkit. Where polkit is not there to ask (a sandbox, a bare session), the key shows
//! without it, since there is nothing to ask.

use gtk::{gio, glib, prelude::*};
use std::collections::HashMap;

const ACTION: &str = "app.kaeri.desktop.show-recovery-key";

/// Run `then` once the account password is given, or at once when polkit cannot ask.
pub fn ask(then: impl FnOnce() + 'static) {
    let then = std::rc::Rc::new(std::cell::RefCell::new(Some(then)));
    let go = {
        let then = then.clone();
        move || {
            if let Some(f) = then.borrow_mut().take() {
                f();
            }
        }
    };
    let Some(start) = start_time() else {
        go();
        return;
    };
    let Ok(bus) = gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) else {
        go();
        return;
    };
    let mut details: HashMap<String, glib::Variant> = HashMap::new();
    details.insert("pid".into(), std::process::id().to_variant());
    details.insert("start-time".into(), start.to_variant());
    let subject = ("unix-process", details).to_variant();
    let args = (subject, ACTION, HashMap::<String, String>::new(), 1u32, "").to_variant();
    bus.call(
        Some("org.freedesktop.PolicyKit1"),
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
        "CheckAuthorization",
        Some(&args),
        None,
        gio::DBusCallFlags::NONE,
        120_000,
        gio::Cancellable::NONE,
        move |reply| match reply {
            Ok(value) => {
                if let Some((authorised, _, _)) =
                    value.get::<((bool, bool, HashMap<String, String>),)>().map(|(r,)| r)
                {
                    if authorised {
                        go();
                    }
                } else {
                    go();
                }
            }
            // No polkit, or no policy for the action installed: nothing to ask.
            Err(_) => go(),
        },
    );
}

/// This process's start time in clock ticks, as polkit names a process.
fn start_time() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after = stat.rsplit_once(") ")?.1;
    after.split_whitespace().nth(19)?.parse().ok()
}
