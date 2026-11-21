//! The device's own lock before the recovery key shows again: the account password, asked
//! through polkit. Where polkit is not there to ask (a sandbox, a bare session, no policy
//! installed for the action), the key shows without it, since there is nothing to ask. Any other
//! failure, a prompt left unanswered among them, keeps the key hidden.

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
        "CheckAuthorization", // i18n-skip
        Some(&args),
        None,
        gio::DBusCallFlags::NONE,
        // No timeout: the password prompt waits for the person, and a timeout must never read
        // as "nothing to ask".
        i32::MAX,
        gio::Cancellable::NONE,
        move |reply| {
            if decide(reply) == Answer::Go {
                go();
            }
        },
    );
}

/// What polkit's reply to CheckAuthorization means for the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// The password was given, or polkit is not there to ask.
    Go,
    /// Polkit said no, or the answer cannot be read.
    Refuse,
}

fn decide(reply: Result<glib::Variant, glib::Error>) -> Answer {
    match reply {
        Ok(value) => match value.get::<((bool, bool, HashMap<String, String>),)>() {
            Some(((true, _, _),)) => Answer::Go,
            _ => Answer::Refuse,
        },
        Err(e) if no_one_to_ask(&e) => Answer::Go,
        Err(_) => Answer::Refuse,
    }
}

/// Polkit is not on the bus, or the action's policy is not installed.
fn no_one_to_ask(e: &glib::Error) -> bool {
    if e.matches(gio::DBusError::ServiceUnknown) || e.matches(gio::DBusError::NameHasNoOwner) {
        return true;
    }
    gio::DBusError::remote_error(e).is_some_and(|name| {
        name.as_str() == "org.freedesktop.PolicyKit1.Error.Failed" // i18n-skip
            && e.message().contains("is not registered") // i18n-skip
    })
}

/// This process's start time in clock ticks, as polkit names a process.
fn start_time() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let after = stat.rsplit_once(") ")?.1;
    after.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(authorised: bool, challenge: bool) -> Result<glib::Variant, glib::Error> {
        Ok(((authorised, challenge, HashMap::<String, String>::new()),).to_variant())
    }

    fn remote(name: &str, message: &str) -> glib::Error {
        glib::Error::new(gio::IOErrorEnum::DbusError, &format!("GDBus.Error:{name}: {message}"))
    }

    #[test]
    fn a_prompt_left_unanswered_keeps_the_key_hidden() {
        let timed_out = glib::Error::new(gio::IOErrorEnum::TimedOut, "Timeout was reached");
        assert_eq!(decide(Err(timed_out)), Answer::Refuse);
        let no_reply = glib::Error::new(gio::DBusError::NoReply, "No reply");
        assert_eq!(decide(Err(no_reply)), Answer::Refuse);
    }

    #[test]
    fn only_a_missing_polkit_or_policy_lets_the_key_show_unasked() {
        let unknown = glib::Error::new(gio::DBusError::ServiceUnknown, "not provided");
        assert_eq!(decide(Err(unknown)), Answer::Go);
        let no_owner = glib::Error::new(gio::DBusError::NameHasNoOwner, "no owner");
        assert_eq!(decide(Err(no_owner)), Answer::Go);
        let no_policy = remote(
            "org.freedesktop.PolicyKit1.Error.Failed",
            "Action app.kaeri.desktop.show-recovery-key is not registered",
        );
        assert_eq!(decide(Err(no_policy)), Answer::Go);
        let cancelled = remote("org.freedesktop.PolicyKit1.Error.Cancelled", "Cancelled");
        assert_eq!(decide(Err(cancelled)), Answer::Refuse);
        let denied = glib::Error::new(gio::DBusError::AccessDenied, "denied");
        assert_eq!(decide(Err(denied)), Answer::Refuse);
    }

    #[test]
    fn only_a_given_password_shows_the_key() {
        assert_eq!(decide(reply(true, false)), Answer::Go);
        assert_eq!(decide(reply(false, false)), Answer::Refuse);
        // No agent to ask for the password.
        assert_eq!(decide(reply(false, true)), Answer::Refuse);
        assert_eq!(decide(Ok(("unexpected",).to_variant())), Answer::Refuse);
    }
}
