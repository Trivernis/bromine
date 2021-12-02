use crate::error_event::ErrorEventData;
use thiserror::Error;
use tokio::sync::oneshot;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] tokio::io::Error),

    #[cfg(feature = "messagepack")]
    #[error(transparent)]
    Decode(#[from] rmp_serde::decode::Error),

    #[cfg(feature = "messagepack")]
    #[error(transparent)]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("Build Error: {0}")]
    BuildError(String),

    #[error("{0}")]
    Message(String),

    #[error("Channel Error: {0}")]
    ReceiveError(#[from] oneshot::error::RecvError),

    #[error("The received event was corrupted")]
    CorruptedEvent,

    #[error("Send Error")]
    SendError,

    #[error("Error response: {0}")]
    ErrorEvent(#[from] ErrorEventData),

    #[error("Timed out")]
    Timeout,
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
