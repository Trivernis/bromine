use thiserror::Error;
use tokio::sync::oneshot;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] tokio::io::Error),

    #[error(transparent)]
    Decode(#[from] rmp_serde::decode::Error),

    #[error(transparent)]
    Encode(#[from] rmp_serde::encode::Error),

    #[error("Build Error: {0}")]
    BuildError(String),

    #[error("{0}")]
    Message(String),

    #[error("Channel Error: {0}")]
    ReceiveError(#[from] oneshot::error::RecvError),

    #[error("Send Error")]
    SendError,
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
