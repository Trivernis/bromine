use crate::context::Context;
use crate::error::Error;
use crate::error_event::ErrorEventData;
use crate::event::{Event, EventType};
use crate::ipc::stream_emitter::emit_metadata::EmitMetadata;
use crate::payload::IntoPayload;
use crate::{error, poll_unwrap};
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

/// A metadata object returned after waiting for a reply to an event
/// This object needs to be awaited for to get the actual reply
pub struct EmitMetadataWithResponse<P: IntoPayload> {
    pub(crate) timeout: Option<Duration>,
    pub(crate) fut: Option<Pin<Box<dyn Future<Output = error::Result<Event>> + Send + Sync>>>,
    pub(crate) emit_metadata: Option<EmitMetadata<P>>,
}

impl<P: IntoPayload> EmitMetadataWithResponse<P> {
    /// Sets a timeout for awaiting replies to this emitted event
    #[inline]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);

        self
    }
}

impl<P: IntoPayload> Unpin for EmitMetadataWithResponse<P> {}

impl<P: IntoPayload + Send + Sync + 'static> Future for EmitMetadataWithResponse<P> {
    type Output = error::Result<Event>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.fut.is_none() {
            let mut emit_metadata = poll_unwrap!(self.emit_metadata.take());
            let ctx = poll_unwrap!(emit_metadata
                .event_metadata
                .as_ref()
                .and_then(|m| m.ctx.clone()));
            let timeout = self
                .timeout
                .take()
                .unwrap_or_else(|| ctx.default_reply_timeout.clone());

            let event_id = match poll_unwrap!(emit_metadata.event_metadata.as_mut()).get_event() {
                Ok(e) => e.id(),
                Err(e) => {
                    return Poll::Ready(Err(e));
                }
            };

            self.fut = Some(Box::pin(async move {
                let mut tx = ctx.register_reply_listener(event_id).await?;
                emit_metadata.await?;

                let reply = tokio::select! {
                    tx_result = tx.recv() => {
                        Ok(tx_result.ok_or_else(|| Error::SendError)?)
                    }
                    _ = tokio::time::sleep(timeout) => {
                        Err(Error::Timeout)
                    }
                }?;

                remove_reply_listener(&ctx, event_id).await;

                if reply.event_type() == EventType::Error {
                    Err(reply.payload::<ErrorEventData>()?.into())
                } else {
                    Ok(reply)
                }
            }))
        }
        self.fut.as_mut().unwrap().as_mut().poll(cx)
    }
}

pub(crate) async fn remove_reply_listener(ctx: &Context, event_id: u64) {
    let mut listeners = ctx.reply_listeners.lock().await;
    listeners.remove(&event_id);
}
