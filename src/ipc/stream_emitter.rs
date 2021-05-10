use crate::error::Result;
use crate::events::event::Event;
use serde::Serialize;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;

/// An abstraction over the raw tokio tcp stream
/// to emit events and share a connection across multiple
/// contexts.
#[derive(Clone)]
pub struct StreamEmitter {
    stream: Arc<Mutex<OwnedWriteHalf>>,
}

impl StreamEmitter {
    pub fn new(stream: OwnedWriteHalf) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }

    /// Emits an event
    pub async fn emit<T: Serialize>(&self, event: &str, data: T) -> Result<()> {
        let data_bytes = rmp_serde::to_vec(&data)?;
        let event = Event::new(event.to_string(), data_bytes);
        let event_bytes = event.to_bytes()?;
        {
            let mut stream = self.stream.lock().await;
            (*stream).write_all(&event_bytes[..]).await?;
        }

        Ok(())
    }
}
