use crate::context::Context;
use crate::error::Error;
use crate::event::EventType;
use crate::ipc::stream_emitter::emit_metadata_with_response::EmitMetadataWithResponse;
use crate::ipc::stream_emitter::event_metadata::EventMetadata;
use crate::ipc::stream_emitter::SendStream;
use crate::payload::IntoPayload;
use crate::{error, poll_unwrap};
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::Poll;
use tokio::io::AsyncWriteExt;

/// A metadata object returned after emitting an event.
/// To send the event this object needs to be awaited
/// This object can be used to wait for a response to an event.
/// The result contains the emitted event id.
pub struct EmitMetadata<P: IntoPayload> {
    pub(crate) event_metadata: Option<EventMetadata<P>>,
    stream: Option<SendStream>,
    fut: Option<Pin<Box<dyn Future<Output = error::Result<u64>> + Send + Sync>>>,
}

impl<P: IntoPayload> EmitMetadata<P> {
    #[inline]
    pub(crate) fn new(
        ctx: Context,
        stream: SendStream,
        event_name: String,
        event_namespace: Option<String>,
        payload: P,
        res_id: Option<u64>,
        event_type: EventType,
    ) -> Self {
        Self {
            event_metadata: Some(EventMetadata {
                event: None,
                ctx: Some(ctx),
                event_name: Some(event_name),
                event_namespace: Some(event_namespace),
                payload: Some(payload),
                res_id: Some(res_id),
                event_type: Some(event_type),
            }),
            stream: Some(stream),
            fut: None,
        }
    }

    /// Waits for a reply to the given message.
    #[tracing::instrument(skip(self), fields(self.message_id))]
    pub fn await_reply(self) -> EmitMetadataWithResponse<P> {
        EmitMetadataWithResponse {
            timeout: None,
            fut: None,
            emit_metadata: Some(self),
        }
    }
}

impl<P: IntoPayload> Unpin for EmitMetadata<P> {}

impl<P: IntoPayload + Send + Sync + 'static> Future for EmitMetadata<P> {
    type Output = error::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.fut.is_none() {
            let event_metadata = poll_unwrap!(self.event_metadata.take());
            let stream = poll_unwrap!(self.stream.take());

            let event = match event_metadata.take_event() {
                Ok(m) => m,
                Err(e) => {
                    return Poll::Ready(Err(e));
                }
            }
            .expect("poll after future was done");

            self.fut = Some(Box::pin(async move {
                let event_id = event.id();
                let event_bytes = event.into_bytes()?;
                let mut stream = stream.lock().await;
                stream.deref_mut().write_all(&event_bytes[..]).await?;
                tracing::trace!(bytes_len = event_bytes.len());

                Ok(event_id)
            }));
        }
        self.fut.as_mut().unwrap().as_mut().poll(cx)
    }
}
