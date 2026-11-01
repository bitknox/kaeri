//! The skip times the viewer marked, outside the player: Settings' "Clear skip times you
//! marked". Marking itself is the playback's, in `playback::session`.

use kaeri_model::{Msg, event::Event, notice::Notice};

use crate::{outbox::Outbox, services::Services};

/// Forget every mark and vote the viewer made, and say how many marks there were.
pub fn clear(services: &Services, tx: &Outbox) {
    let notice = match services.store.clear_skip_marks() {
        Ok(0) => Notice::info(Msg::NoticeSkipMarksNone),
        Ok(count) => Notice::info(Msg::NoticeSkipMarksCleared { count: i64::from(count) }),
        Err(error) => {
            Notice::alert(Msg::NoticeSkipMarksNotCleared { reason: error.to_string() })
        }
    };
    let _ = tx.send(Event::Toast(notice));
}
