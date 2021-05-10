use serde::{Deserialize, Serialize};

pub static ERROR_EVENT_NAME: &str = "error";

/// Data returned on error event.
/// The error event has a default handler that just logs that
/// an error occurred. For a custom handler, register a handler on
/// the [ERROR_EVENT_NAME] event.
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct ErrorEventData {
    pub code: u16,
    pub message: String,
}
