use std::future::Future;
use std::mem;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use futures::future;
use futures::future::Either;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing;

use crate::error::{Error, Result};
use crate::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::event::EventType;
use crate::events::event::Event;
use crate::ipc::context::Context;
use crate::payload::IntoPayload;
use crate::protocol::AsyncProtocolStream;

macro_rules! poll_unwrap {
    ($val:expr) => {
        if let Some(v) = $val {
            v
        } else {
            tracing::error!("Polling a future with an invalid state.");
            return Poll::Ready(Err(Error::InvalidState));
        }
    };
}

type SendStream = Arc<Mutex<dyn AsyncWrite + Send + Sync + Unpin + 'static>>;

/// An abstraction over any type that implements the AsyncProtocolStream trait
/// to emit events and share a connection across multiple
/// contexts.
#[derive(Clone)]
pub struct StreamEmitter {
    stream: SendStream,
}

impl StreamEmitter {
    pub fn new<P: AsyncProtocolStream + 'static>(stream: P::OwnedSplitWriteHalf) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }

    #[tracing::instrument(level = "trace", skip(self, ctx, payload))]
    fn _emit<P: IntoPayload>(
        &self,
        ctx: Context,
        namespace: Option<&str>,
        event: &str,
        payload: P,
        res_id: Option<u64>,
        event_type: EventType,
    ) -> EmitMetadata<P> {
        EmitMetadata::new(
            ctx,
            self.stream.clone(),
            event.to_string(),
            namespace.map(|n| n.to_string()),
            payload,
            res_id,
            event_type,
        )
    }

    /// Emits an event
    #[inline]
    pub(crate) fn emit<S: AsRef<str>, P: IntoPayload>(
        &self,
        ctx: Context,
        event: S,
        payload: P,
    ) -> EmitMetadata<P> {
        self._emit(
            ctx,
            None,
            event.as_ref(),
            payload,
            None,
            EventType::Initiator,
        )
    }

    /// Emits an event to a specific namespace
    #[inline]
    pub(crate) fn emit_to<S1: AsRef<str>, S2: AsRef<str>, P: IntoPayload>(
        &self,
        ctx: Context,
        namespace: S1,
        event: S2,
        payload: P,
    ) -> EmitMetadata<P> {
        self._emit(
            ctx,
            Some(namespace.as_ref()),
            event.as_ref(),
            payload,
            None,
            EventType::Initiator,
        )
    }

    /// Emits a response to an event
    #[inline]
    pub(crate) fn emit_response<S: AsRef<str>, P: IntoPayload>(
        &self,
        ctx: Context,
        event_id: u64,
        event: S,
        payload: P,
    ) -> EmitMetadata<P> {
        self._emit(
            ctx,
            None,
            event.as_ref(),
            payload,
            Some(event_id),
            EventType::Response,
        )
    }

    /// Emits a response to an event to a namespace
    #[inline]
    pub(crate) fn emit_response_to<S1: AsRef<str>, S2: AsRef<str>, P: IntoPayload>(
        &self,
        ctx: Context,
        event_id: u64,
        namespace: S1,
        event: S2,
        payload: P,
    ) -> EmitMetadata<P> {
        self._emit(
            ctx,
            Some(namespace.as_ref()),
            event.as_ref(),
            payload,
            Some(event_id),
            EventType::Response,
        )
    }
}

struct EventMetadata<P: IntoPayload> {
    event: Option<Event>,
    ctx: Option<Context>,
    event_namespace: Option<Option<String>>,
    event_name: Option<String>,
    res_id: Option<Option<u64>>,
    event_type: Option<EventType>,
    payload: Option<P>,
}

impl<P: IntoPayload> EventMetadata<P> {
    pub fn get_event(&mut self) -> Result<&Event> {
        if self.event.is_none() {
            self.build_event()?;
        }
        Ok(self.event.as_ref().unwrap())
    }

    pub fn take_event(mut self) -> Result<Option<Event>> {
        if self.event.is_none() {
            self.build_event()?;
        }
        Ok(mem::take(&mut self.event))
    }

    fn build_event(&mut self) -> Result<()> {
        let ctx = self.ctx.take().ok_or(Error::InvalidState)?;
        let event = self.event_name.take().ok_or(Error::InvalidState)?;
        let namespace = self.event_namespace.take().ok_or(Error::InvalidState)?;
        let payload = self.payload.take().ok_or(Error::InvalidState)?;
        let res_id = self.res_id.take().ok_or(Error::InvalidState)?;
        let event_type = self.event_type.take().ok_or(Error::InvalidState)?;
        let payload_bytes = payload.into_payload(&ctx)?;

        let event = Event::new(
            namespace,
            event.to_string(),
            payload_bytes,
            res_id,
            event_type,
        );

        self.event = Some(event);

        Ok(())
    }
}

/// A metadata object returned after emitting an event.
/// To send the event this object needs to be awaited
/// This object can be used to wait for a response to an event.
/// The result contains the emitted event id.
pub struct EmitMetadata<P: IntoPayload> {
    event_metadata: Option<EventMetadata<P>>,
    stream: Option<SendStream>,
    fut: Option<Pin<Box<dyn Future<Output = Result<u64>> + Send + Sync>>>,
}

/// A metadata object returned after waiting for a reply to an event
/// This object needs to be awaited for to get the actual reply
pub struct EmitMetadataWithResponse<P: IntoPayload> {
    timeout: Option<Duration>,
    fut: Option<Pin<Box<dyn Future<Output = Result<Event>> + Send + Sync>>>,
    emit_metadata: Option<EmitMetadata<P>>,
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

impl<P: IntoPayload> EmitMetadataWithResponse<P> {
    /// Sets a timeout for awaiting replies to this emitted event
    #[inline]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);

        self
    }
}

impl<P: IntoPayload> Unpin for EmitMetadata<P> {}

impl<P: IntoPayload> Unpin for EmitMetadataWithResponse<P> {}

impl<P: IntoPayload + Send + Sync + 'static> Future for EmitMetadata<P> {
    type Output = Result<u64>;

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

impl<P: IntoPayload + Send + Sync + 'static> Future for EmitMetadataWithResponse<P> {
    type Output = Result<Event>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.fut.is_none() {
            let mut emit_metadata = poll_unwrap!(self.emit_metadata.take());
            let ctx = poll_unwrap!(emit_metadata
                .event_metadata
                .as_ref()
                .and_then(|m| m.ctx.clone()));
            let timeout = self
                .timeout
                .clone()
                .unwrap_or(ctx.default_reply_timeout.clone());

            let event_id = match poll_unwrap!(emit_metadata.event_metadata.as_mut()).get_event() {
                Ok(e) => e.id(),
                Err(e) => {
                    return Poll::Ready(Err(e));
                }
            };

            self.fut = Some(Box::pin(async move {
                let tx = ctx.register_reply_listener(event_id).await?;
                emit_metadata.await?;

                let result =
                    future::select(Box::pin(tx), Box::pin(tokio::time::sleep(timeout))).await;

                let reply = match result {
                    Either::Left((tx_result, _)) => Ok(tx_result?),
                    Either::Right(_) => {
                        let mut listeners = ctx.reply_listeners.lock().await;
                        listeners.remove(&event_id);
                        Err(Error::Timeout)
                    }
                }?;
                if reply.name() == ERROR_EVENT_NAME {
                    Err(reply.payload::<ErrorEventData>()?.into())
                } else {
                    Ok(reply)
                }
            }))
        }
        self.fut.as_mut().unwrap().as_mut().poll(cx)
    }
}
