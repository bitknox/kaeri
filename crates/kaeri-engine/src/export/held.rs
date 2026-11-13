//! Sends under way, kept in the store so one left waiting for Wi-Fi, or cut off by the app
//! being closed while it prepared, is started again at the next launch, as a queued chapter
//! download is. One cut off a second time, as a send that takes the app down would be, ends
//! with a failure the screen shows instead of starting again at every launch.

use std::sync::Mutex;

use kaeri_core::books::{ChapterId, WorkId};
use kaeri_model::reading::{DownloadPick, ExportFormat, ReadingCommand};
use kaeri_store::Store;
use serde::{Deserialize, Serialize};

/// The store meta key the sends under way are kept under, as a JSON list.
const KEY: &str = "export:held";

/// One read and write of the list at a time: sends wait and go on side by side.
static LIST: Mutex<()> = Mutex::new(());

/// A send as it was asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HeldSend {
    /// Which send this is, for taking it off the list.
    pub key: String,
    pub work: String,
    pub pick: Pick,
    pub target: String,
    pub format: Format,
    /// Fetching or writing, rather than waiting for Wi-Fi: one kept this way when the engine
    /// stopped was cut off by the app being closed.
    #[serde(default)]
    pub running: bool,
    /// How many times this send was started again after being cut off.
    #[serde(default)]
    pub cut: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Pick {
    NextUnread { n: u32 },
    Unread,
    Volume { volume: String },
    Chapters { ids: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Format {
    FixedEpub,
    Kepub,
    Epub,
    Cbz,
}

impl From<&DownloadPick> for Pick {
    fn from(pick: &DownloadPick) -> Self {
        match pick {
            DownloadPick::NextUnread { n } => Self::NextUnread { n: *n },
            DownloadPick::Unread => Self::Unread,
            DownloadPick::Volume { volume } => Self::Volume { volume: volume.clone() },
            DownloadPick::Chapters { ids } => {
                Self::Chapters { ids: ids.iter().map(|c| c.as_str().to_owned()).collect() }
            }
        }
    }
}

impl From<ExportFormat> for Format {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::FixedEpub => Self::FixedEpub,
            ExportFormat::Kepub => Self::Kepub,
            ExportFormat::Epub => Self::Epub,
            ExportFormat::Cbz => Self::Cbz,
        }
    }
}

impl HeldSend {
    /// The command that asks for this send again.
    pub fn command(&self) -> ReadingCommand {
        let pick = match &self.pick {
            Pick::NextUnread { n } => DownloadPick::NextUnread { n: *n },
            Pick::Unread => DownloadPick::Unread,
            Pick::Volume { volume } => DownloadPick::Volume { volume: volume.clone() },
            Pick::Chapters { ids } => {
                DownloadPick::Chapters { ids: ids.iter().map(ChapterId::new).collect() }
            }
        };
        let format = match self.format {
            Format::FixedEpub => ExportFormat::FixedEpub,
            Format::Kepub => ExportFormat::Kepub,
            Format::Epub => ExportFormat::Epub,
            Format::Cbz => ExportFormat::Cbz,
        };
        ReadingCommand::Prepare {
            work: WorkId::new(self.work.clone()),
            pick,
            target: self.target.clone(),
            format,
        }
    }
}

fn read(store: &Store) -> Vec<HeldSend> {
    store
        .get_meta_text(KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn write(store: &Store, list: &[HeldSend]) {
    let written = if list.is_empty() {
        store.delete_meta(KEY)
    } else {
        match serde_json::to_string(list) {
            Ok(json) => store.set_meta_text(KEY, &json),
            Err(_) => return,
        }
    };
    if let Err(e) = written {
        tracing::warn!(error = %e, "the sends waiting for Wi-Fi were not kept");
    }
}

/// Keep `send` as under way, waiting or running as it says.
pub(crate) fn keep(store: &Store, send: HeldSend) {
    let _list = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = read(store);
    list.retain(|s| s.key != send.key);
    list.push(send);
    write(store, &list);
}

/// The send under `key` is no longer under way: it is ready, was cancelled or failed.
pub(crate) fn forget(store: &Store, key: &str) {
    let _list = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = read(store);
    let before = list.len();
    list.retain(|s| s.key != key);
    if list.len() != before {
        write(store, &list);
    }
}

/// The sends left under way, taken off the list: each is asked for again, and keeps itself
/// once more as it goes.
pub(crate) fn take(store: &Store) -> Vec<HeldSend> {
    let _list = LIST.lock().unwrap_or_else(|e| e.into_inner());
    let list = read(store);
    if !list.is_empty() {
        write(store, &[]);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waiting_send_is_kept_until_it_is_forgotten_or_taken() {
        let store = Store::open_in_memory().unwrap();
        // Kept as a list from before sends said whether they ran: each reads as waiting.
        let send = |key: &str| -> HeldSend {
            let pick = Pick::from(&DownloadPick::Chapters { ids: vec![ChapterId::new("2")] });
            serde_json::from_value(serde_json::json!({
                "key": key,
                "work": "a:900501",
                "pick": serde_json::to_value(pick).unwrap(),
                "target": "kobo",
                "format": serde_json::to_value(Format::from(ExportFormat::Kepub)).unwrap(),
            }))
            .unwrap()
        };
        keep(&store, send("1"));
        keep(&store, send("2"));
        forget(&store, "1");
        let taken = take(&store);
        assert_eq!(taken, vec![send("2")]);
        assert!(take(&store).is_empty(), "taken once");
        let ReadingCommand::Prepare { pick, format, target, .. } = taken[0].command() else {
            panic!("not a send")
        };
        assert_eq!(pick, DownloadPick::Chapters { ids: vec![ChapterId::new("2")] });
        assert_eq!((format, target.as_str()), (ExportFormat::Kepub, "kobo"));
    }
}
