use crate::error::Result;
use crate::events::event::Event;
use crate::events::payload::EventSendPayload;
use crate::ipc::context::Context;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;
use tokio::time::Instant;

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

    pub async fn _emit<T: EventSendPayload>(
        &self,
        namespace: Option<&str>,
        event: &str,
        data: T,
        res_id: Option<u64>,
    ) -> Result<EmitMetadata> {
        let data_bytes = data.to_payload_bytes()?;
        log::debug!("Emitting event {:?}:{}", namespace, event);

        let event = if let Some(namespace) = namespace {
            Event::with_namespace(namespace.to_string(), event.to_string(), data_bytes, res_id)
        } else {
            Event::new(event.to_string(), data_bytes, res_id)
        };

        let event_id = event.id();

        let event_bytes = event.into_bytes()?;
        {
            let start = Instant::now();
            let mut stream = self.stream.lock().await;
            (*stream).write_all(&event_bytes[..]).await?;
            log::trace!("Wrote {} bytes in {:?}", event_bytes.len(), start.elapsed());
        }

        Ok(EmitMetadata::new(event_id))
    }

    /// Emits an event
    pub async fn emit<S: AsRef<str>, T: EventSendPayload>(
        &self,
        event: S,
        data: T,
    ) -> Result<EmitMetadata> {
        self._emit(None, event.as_ref(), data, None).await
    }

    /// Emits an event to a specific namespace
    pub async fn emit_to<S1: AsRef<str>, S2: AsRef<str>, T: EventSendPayload>(
        &self,
        namespace: S1,
        event: S2,
        data: T,
    ) -> Result<EmitMetadata> {
        self._emit(Some(namespace.as_ref()), event.as_ref(), data, None)
            .await
    }

    /// Emits a response to an event
    pub async fn emit_response<S: AsRef<str>, T: EventSendPayload>(
        &self,
        event_id: u64,
        event: S,
        data: T,
    ) -> Result<EmitMetadata> {
        self._emit(None, event.as_ref(), data, Some(event_id)).await
    }

    /// Emits a response to an event to a namespace
    pub async fn emit_response_to<S1: AsRef<str>, S2: AsRef<str>, T: EventSendPayload>(
        &self,
        event_id: u64,
        namespace: S1,
        event: S2,
        data: T,
    ) -> Result<EmitMetadata> {
        self._emit(
            Some(namespace.as_ref()),
            event.as_ref(),
            data,
            Some(event_id),
        )
        .await
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
