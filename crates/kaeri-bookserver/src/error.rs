use kaeri_core::error::ProviderError;

/// Why a server did not give what was asked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServerError {
    /// Nothing answered: DNS, a refused connection, a timeout, a certificate that is not trusted.
    #[error("could not reach the server: {0}")]
    Unreachable(String),
    /// The server answered 401 or 403 to the sign-in this device holds.
    #[error("the server does not accept the sign-in")]
    Unauthorized,
    #[error("the server has no such thing")]
    NotFound,
    /// Any other status.
    #[error("the server answered {0}")]
    Status(u16),
    /// The answer was not what this kind of server sends.
    #[error("the server's answer was not understood: {0}")]
    Parse(String),
    /// The address answers, but not as a book server.
    #[error("that address is not a book server")]
    NotABookServer,
    /// A file could not be kept on this device.
    #[error("could not keep the file: {0}")]
    Disk(String),
}

impl ServerError {
    /// Whether this is a sign-in problem the person fixes by signing in again.
    pub const fn needs_sign_in(&self) -> bool {
        matches!(self, Self::Unauthorized)
    }
}

impl From<ServerError> for ProviderError {
    fn from(error: ServerError) -> Self {
        match error {
            ServerError::Unreachable(reason) => Self::Transport(reason),
            // The server is up and refuses this device: not a health fault, a sign-in to redo.
            ServerError::Unauthorized => Self::Unavailable("signed out".into()),
            ServerError::NotFound => Self::NotFound,
            ServerError::Status(status) => {
                Self::Blocked(format!("the server answered {status}"))
            }
            ServerError::Parse(reason) => Self::Parse(reason),
            ServerError::NotABookServer => Self::Parse("not a book server".into()),
            ServerError::Disk(reason) => Self::Other(reason),
        }
    }
}

pub type Result<T, E = ServerError> = std::result::Result<T, E>;
