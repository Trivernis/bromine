use crate::context::Context;
use crate::error::{Error, Result};
use crate::event::{Event, EventType};
use crate::ipc::stream_emitter::emit_metadata::EmitMetadata;
use crate::ipc::stream_emitter::emit_metadata_with_response::remove_reply_listener;
use crate::payload::IntoPayload;
use crate::poll_unwrap;
use futures_core::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

/// A metadata object returned after waiting for a reply to an event
/// This object needs to be awaited for to get the actual reply
pub struct EmitMetadataWithResponseStream<P: IntoPayload> {
    pub(crate) timeout: Option<Duration>,
    pub(crate) fut: Option<Pin<Box<dyn Future<Output = Result<ResponseStream>> + Send + Sync>>>,
    pub(crate) emit_metadata: Option<EmitMetadata<P>>,
}

/// An asynchronous stream one can read all responses to a specific event from.
pub struct ResponseStream {
    event_id: u64,
    ctx: Option<Context>,
    receiver: Option<Receiver<Event>>,
    timeout: Duration,
    fut: Option<Pin<Box<dyn Future<Output = Result<(Option<Event>, Context, Receiver<Event>)>>>>>,
}

impl ResponseStream {
    pub(crate) fn new(
        event_id: u64,
        timeout: Duration,
        ctx: Context,
        receiver: Receiver<Event>,
    ) -> Self {
        Self {
            event_id,
            ctx: Some(ctx),
            receiver: Some(receiver),
            timeout,
            fut: None,
        }
    }
}

impl<P: IntoPayload> Unpin for EmitMetadataWithResponseStream<P> {}

impl<P: IntoPayload> EmitMetadataWithResponseStream<P> {
    /// Sets a timeout for awaiting replies to this emitted event
    #[inline]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);

        self
    }
}

impl<P: IntoPayload + Send + Sync + 'static> Future for EmitMetadataWithResponseStream<P> {
    type Output = Result<ResponseStream>;

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
                let tx = ctx.register_reply_listener(event_id).await?;
                emit_metadata.await?;

                Ok(ResponseStream::new(event_id, timeout, ctx, tx))
            }))
        }
        self.fut.as_mut().unwrap().as_mut().poll(cx)
    }
}

impl Unpin for ResponseStream {}

impl Stream for ResponseStream {
    type Item = Result<Event>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.fut.is_none() {
            if self.ctx.is_none() || self.receiver.is_none() {
                return Poll::Ready(None);
            }
            let ctx = self.ctx.take().unwrap();
            let mut receiver = self.receiver.take().unwrap();
            let timeout = self.timeout;
            let event_id = self.event_id;

            self.fut = Some(Box::pin(async move {
                let event: Option<Event> = tokio::select! {
                    tx_result = receiver.recv() => {
                        Ok(tx_result)
                    }
                    _ = tokio::time::sleep(timeout) => {
                        Err(Error::Timeout)
                    }
                }?;

                if event.is_none() || event.as_ref().unwrap().event_type() == EventType::End {
                    remove_reply_listener(&ctx, event_id).await;
                }

                Ok((event, ctx, receiver))
            }));
        }

        match self.fut.as_mut().unwrap().as_mut().poll(cx) {
            Poll::Ready(r) => match r {
                Ok((event, ctx, tx)) => {
                    self.fut = None;

                    if let Some(event) = event {
                        if event.event_type() != EventType::End {
                            self.ctx = Some(ctx);
                            self.receiver = Some(tx);
                        }

                        Poll::Ready(Some(Ok(event)))
                    } else {
                        Poll::Ready(None)
                    }
                }
                Err(e) => Poll::Ready(Some(Err(e))),
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
