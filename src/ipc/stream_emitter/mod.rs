pub mod emit_metadata;
pub mod emit_metadata_with_response;
mod event_metadata;

use std::future::Future;
use std::mem;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use emit_metadata::EmitMetadata;
use event_metadata::EventMetadata;
use futures::future;
use futures::future::Either;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing;

use crate::error::Result;
use crate::event::EventType;
use crate::ipc::context::Context;
use crate::payload::IntoPayload;
use crate::protocol::AsyncProtocolStream;

#[macro_export]
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
        namespace: Option<String>,
        event: &str,
        payload: P,
        res_id: Option<u64>,
        event_type: EventType,
    ) -> EmitMetadata<P> {
        EmitMetadata::new(
            ctx,
            self.stream.clone(),
            event.to_string(),
            namespace,
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
            Some(namespace.as_ref().to_string()),
            event.as_ref(),
            payload,
            None,
            EventType::Initiator,
        )
    }

    /// Emits a raw event
    #[inline]
    pub(crate) fn emit_raw<P: IntoPayload>(
        &self,
        ctx: Context,
        res_id: Option<u64>,
        event: &str,
        namespace: Option<String>,
        event_type: EventType,
        payload: P,
    ) -> EmitMetadata<P> {
        self._emit(ctx, namespace, event, payload, res_id, event_type)
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
            Some(namespace.as_ref().to_string()),
            event.as_ref(),
            payload,
            Some(event_id),
            EventType::Response,
        )
    }
}
