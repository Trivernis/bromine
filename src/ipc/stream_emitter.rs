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

    pub async fn _emit<T: Serialize>(
        &self,
        event: &str,
        data: T,
        res_id: Option<u64>,
    ) -> Result<()> {
        let data_bytes = rmp_serde::to_vec(&data)?;
        let event = Event::new(event.to_string(), data_bytes, res_id);
        let event_bytes = event.to_bytes()?;
        {
            let mut stream = self.stream.lock().await;
            (*stream).write_all(&event_bytes[..]).await?;
        }

        Ok(())
    }

    /// Emits an event
    pub async fn emit<T: Serialize>(&self, event: &str, data: T) -> Result<()> {
        self._emit(event, data, None).await?;

        Ok(())
    }

    /// Emits a response to an event
    pub async fn emit_response<T: Serialize>(
        &self,
        event_id: u64,
        event: &str,
        data: T,
    ) -> Result<()> {
        self._emit(event, data, Some(event_id)).await?;

        Ok(())
    }
}
