use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("pairing failed: {0}")]
    Pairing(String),
    /// It exists, but cannot be had right now (e.g. a file that is on
    /// another device which is not reachable).
    #[error("not available right now: {0}")]
    Unavailable(String),
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("document error: {0}")]
    Doc(#[from] automerge::AutomergeError),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl Error {
    /// Short machine-readable code, used by the SDK, REST API and MCP server.
    pub fn code(&self) -> &'static str {
        match self {
            Error::PermissionDenied(_) => "permission_denied",
            Error::NotFound(_) => "not_found",
            Error::Invalid(_) => "invalid",
            Error::Pairing(_) => "pairing_failed",
            Error::Unavailable(_) => "unavailable",
            Error::Storage(_) | Error::Doc(_) | Error::Other(_) => "internal",
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Other(e.into())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
