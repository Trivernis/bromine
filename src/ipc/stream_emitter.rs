use crate::context::Context;
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
    ) -> Result<EmitMetadata> {
        let data_bytes = rmp_serde::to_vec(&data)?;
        let event = Event::new(event.to_string(), data_bytes, res_id);
        let event_bytes = event.to_bytes()?;
        {
            let mut stream = self.stream.lock().await;
            (*stream).write_all(&event_bytes[..]).await?;
        }

        Ok(EmitMetadata::new(event.id()))
    }

    /// Emits an event
    pub async fn emit<T: Serialize>(&self, event: &str, data: T) -> Result<EmitMetadata> {
        let metadata = self._emit(event, data, None).await?;

        Ok(metadata)
    }

    /// Emits a response to an event
    pub async fn emit_response<T: Serialize>(
        &self,
        event_id: u64,
        event: &str,
        data: T,
    ) -> Result<EmitMetadata> {
        let metadata = self._emit(event, data, Some(event_id)).await?;

        Ok(metadata)
    }
}

/// A metadata object returned after emitting an event.
/// This object can be used to wait for a response to an event.
pub struct EmitMetadata {
    message_id: u64,
}

impl EmitMetadata {
    pub(crate) fn new(message_id: u64) -> Self {
        Self { message_id }
    }

    /// The ID of the emitted message
    pub fn message_id(&self) -> u64 {
        self.message_id
    }

    /// Waits for a reply to the given message.
    pub async fn await_reply(&self, ctx: &Context) -> Result<Event> {
        let reply = ctx.await_reply(self.message_id).await?;
        Ok(reply)
    }
}
