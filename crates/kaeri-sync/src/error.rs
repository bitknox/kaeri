use crate::folder::FolderError;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Folder(#[from] FolderError),
    #[error(transparent)]
    Store(#[from] kaeri_store::StoreError),
    /// A file could not be opened with any key this device holds: the group key was rotated
    /// without this device, or the folder holds another group's files.
    #[error("this device does not hold the key for it")]
    KeyMismatch,
    /// The recovery key does not open this folder's group.
    #[error("that recovery key does not open this folder")]
    WrongRecoveryKey,
    #[error("the recovery key is not 24 letters and digits")]
    BadRecoveryKey,
    #[error("the folder holds no sync group")]
    NoGroup,
    #[error("the folder already holds a sync group")]
    GroupExists,
    #[error("a file in the folder is damaged or not Kaeri's: {0}")]
    Damaged(&'static str),
    #[error("this device was removed from the group")]
    Removed,
    #[error("{0}")]
    Pairing(String),
    #[error("{0}")]
    Network(String),
}

pub type Result<T> = std::result::Result<T, SyncError>;
