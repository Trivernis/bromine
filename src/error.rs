use crate::error_event::ErrorEventData;
use thiserror::Error;
use tokio::sync::oneshot;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] tokio::io::Error),

    #[cfg(feature = "serialize")]
    #[error("failed to serialize event: {0}")]
    Serialization(#[from] crate::payload::SerializationError),

    #[error("build Error: {0}")]
    BuildError(String),

    #[error("{0}")]
    Message(String),

    #[error("channel Error: {0}")]
    ReceiveError(#[from] oneshot::error::RecvError),

    #[error("the received event was corrupted")]
    CorruptedEvent,

    #[error("send Error")]
    SendError,

    #[error("received error response: {0}")]
    ErrorEvent(#[from] ErrorEventData),

    #[error("timed out")]
    Timeout,

    #[error("Unsupported API Version {0}")]
    UnsupportedVersion(String),

    #[error("Invalid state")]
    InvalidState,
}

impl Error {
    pub fn unsupported_version_vec(version: Vec<u8>) -> Self {
        let mut version_string = version
            .into_iter()
            .fold(String::new(), |acc, val| format!("{}{}.", acc, val));
        version_string.pop();

        Self::UnsupportedVersion(version_string)
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Message(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::Message(s.to_string())
    }
}
