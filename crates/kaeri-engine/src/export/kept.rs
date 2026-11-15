//! Ready sends kept across a restart. A ready send's files stay in its folder until it is put
//! away or [`super::READY_KEPT`] passes. Beside them the folder keeps what the send was, so a
//! send that was ready when the app stopped (or was killed) is offered again at the next
//! launch, and the launch sweep leaves its folder for as long as it is kept.

use std::path::Path;

use kaeri_core::books::{ChapterId, WorkId};
use kaeri_model::reading::ExportFile;
use serde::{Deserialize, Serialize};

/// The file in a ready send's folder that says what the send was.
pub(crate) const RECORD: &str = ".kaeri-send.json";

/// A ready send as its folder keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KeptSend {
    pub work: String,
    pub target: String,
    pub chapters: Vec<String>,
    pub files: Vec<KeptFile>,
    /// When it became ready, in seconds since 1970 on the system clock.
    pub at: i64,
}

/// One file of a kept send, by its path inside the send's folder: the folder itself may move
/// between launches, as an app's container does on iOS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct KeptFile {
    pub inside: String,
    pub name: String,
    pub bytes: u64,
    pub part: Option<u32>,
}

/// Seconds since 1970 on the system clock.
pub(crate) fn wall_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl KeptSend {
    pub(crate) fn new(
        work: &WorkId,
        target: &str,
        chapters: &[ChapterId],
        folder: &Path,
        files: &[ExportFile],
    ) -> Option<Self> {
        let files = files
            .iter()
            .map(|f| {
                let inside = Path::new(&f.path).strip_prefix(folder).ok()?;
                Some(KeptFile {
                    inside: inside.to_str()?.to_owned(),
                    name: f.name.clone(),
                    bytes: f.bytes,
                    part: f.part,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            work: work.as_str().to_owned(),
            target: target.to_owned(),
            chapters: chapters.iter().map(|c| c.as_str().to_owned()).collect(),
            files,
            at: wall_now(),
        })
    }

    /// Keep this send beside its files in `folder`.
    pub(crate) fn write(&self, folder: &Path) {
        let written = serde_json::to_vec(self)
            .map_err(std::io::Error::other)
            .and_then(|json| std::fs::write(folder.join(RECORD), json));
        if let Err(e) = written {
            tracing::warn!(error = %e, "a ready send was not kept for the next launch");
        }
    }

    /// The send `folder` keeps, while it is kept: younger than [`super::READY_KEPT`] at `now`
    /// (seconds since 1970) and with every file still there.
    pub(crate) fn read(folder: &Path, now: i64) -> Option<Self> {
        let json = std::fs::read(folder.join(RECORD)).ok()?;
        let send: Self = serde_json::from_slice(&json).ok()?;
        let age = now.checked_sub(send.at)?;
        if !(0..super::READY_KEPT).contains(&age) || send.files.is_empty() {
            return None;
        }
        let whole = send.files.iter().all(|f| {
            let inside = Path::new(&f.inside);
            inside.components().all(|c| matches!(c, std::path::Component::Normal(_)))
                && folder.join(inside).is_file()
        });
        whole.then_some(send)
    }

    /// The send's files, where `folder` holds them now.
    pub(crate) fn files_in(&self, folder: &Path) -> Vec<ExportFile> {
        self.files
            .iter()
            .map(|f| ExportFile {
                path: folder.join(&f.inside).display().to_string(),
                name: f.name.clone(),
                bytes: f.bytes,
                part: f.part,
            })
            .collect()
    }
}
