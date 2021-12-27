use std::collections::HashMap;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::future;
use futures::future::Either;
use tokio::sync::{Mutex, oneshot, RwLock};
use tokio::sync::oneshot::Sender;
use tokio::time::Duration;
use typemap_rev::TypeMap;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::ipc::stream_emitter::{EmitMetadata, StreamEmitter};
#[cfg(feature = "serialize")]
use crate::payload::{DynamicSerializer, SerdePayload};
use crate::payload::IntoPayload;

pub(crate) type ReplyListeners = Arc<Mutex<HashMap<u64, oneshot::Sender<Event>>>>;

/// An object provided to each callback function.
/// Currently it only holds the event emitter to emit response events in event callbacks.
/// ```rust
/// use bromine::prelude::*;
///
/// async fn my_callback(ctx: &Context, _event: Event) -> IPCResult<()> {
///     // use the emitter on the context object to emit events
///     // inside callbacks
///     ctx.emit("ping", ()).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Context {
    /// The event emitter
    emitter: StreamEmitter,

    /// Field to store additional context data
    pub data: Arc<RwLock<TypeMap>>,

    stop_sender: Arc<Mutex<Option<Sender<()>>>>,

    reply_listeners: ReplyListeners,

    reply_timeout: Duration,

    ref_id: Option<u64>,

    #[cfg(feature = "serialize")]
    pub default_serializer: DynamicSerializer,
}

impl Context {
    pub(crate) fn new(
        emitter: StreamEmitter,
        data: Arc<RwLock<TypeMap>>,
        stop_sender: Option<Sender<()>>,
        reply_listeners: ReplyListeners,
        reply_timeout: Duration,
        #[cfg(feature = "serialize")] default_serializer: DynamicSerializer,
    ) -> Self {
        Self {
            emitter,
            reply_listeners,
            data,
            stop_sender: Arc::new(Mutex::new(stop_sender)),
            reply_timeout,
            #[cfg(feature = "serialize")]
            default_serializer,
            ref_id: None,
        }
    }

    /// Emits an event with a given payload that can be serialized into bytes
    pub async fn emit<S: AsRef<str>, P: IntoPayload>(
        &self,
        name: S,
        payload: P,
    ) -> Result<EmitMetadata> {
        let payload_bytes = payload.into_payload(&self)?;

        if let Some(ref_id) = &self.ref_id {
            self.emitter
                .emit_response(*ref_id, name, payload_bytes)
                .await
        } else {
            self.emitter.emit(name, payload_bytes).await
        }
    }

    /// Emits an event to a specific namespace
    pub async fn emit_to<S1: AsRef<str>, S2: AsRef<str>, P: IntoPayload>(
        &self,
        namespace: S1,
        name: S2,
        payload: P,
    ) -> Result<EmitMetadata> {
        let payload_bytes = payload.into_payload(&self)?;

        if let Some(ref_id) = &self.ref_id {
            self.emitter
                .emit_response_to(*ref_id, namespace, name, payload_bytes)
                .await
        } else {
            self.emitter.emit_to(namespace, name, payload_bytes).await
        }
    }

    /// Waits for a reply to the given message ID
    #[inline]
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn await_reply(&self, message_id: u64) -> Result<Event> {
        self.await_reply_with_timeout(message_id, self.reply_timeout.to_owned()).await
    }

    /// Waits for a reply to the given Message ID with a given timeout
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn await_reply_with_timeout(&self, message_id: u64, timeout: Duration) -> Result<Event> {
        let (rx, tx) = oneshot::channel();
        {
            let mut listeners = self.reply_listeners.lock().await;
            listeners.insert(message_id, rx);
        }

        let result = future::select(
            Box::pin(tx),
            Box::pin(tokio::time::sleep(timeout)),
        )
            .await;

        let event = match result {
            Either::Left((tx_result, _)) => Ok(tx_result?),
            Either::Right(_) => {
                let mut listeners = self.reply_listeners.lock().await;
                listeners.remove(&message_id);
                Err(Error::Timeout)
            }
        }?;

        Ok(event)
    }

    /// Stops the listener and closes the connection
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn stop(self) -> Result<()> {
        let mut sender = self.stop_sender.lock().await;
        if let Some(sender) = mem::take(&mut *sender) {
            sender.send(()).map_err(|_| Error::SendError)?;
        }

        Ok(())
    }

    #[cfg(feature = "serialize")]
    #[inline]
    pub fn create_serde_payload<T>(&self, data: T) -> SerdePayload<T> {
        SerdePayload::new(self.default_serializer.clone(), data)
    }

    /// Returns the channel for a reply to the given message id
    #[inline]
    pub(crate) async fn get_reply_sender(&self, ref_id: u64) -> Option<oneshot::Sender<Event>> {
        let mut listeners = self.reply_listeners.lock().await;
        listeners.remove(&ref_id)
    }

    #[inline]
    pub(crate) fn set_ref_id(&mut self, id: Option<u64>) {
        self.ref_id = id;
    }
}

#[derive(Clone)]
pub struct PooledContext {
    contexts: Vec<PoolGuard<Context>>,
}

pub struct PoolGuard<T>
    where
        T: Clone,
{
    inner: T,
    count: Arc<AtomicUsize>,
}

impl<T> Deref for PoolGuard<T>
    where
        T: Clone,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for PoolGuard<T>
    where
        T: Clone,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> Clone for PoolGuard<T>
    where
        T: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        self.acquire();

        Self {
            inner: self.inner.clone(),
            count: Arc::clone(&self.count),
        }
    }
}

impl<T> Drop for PoolGuard<T>
    where
        T: Clone,
{
    #[inline]
    fn drop(&mut self) {
        self.release();
    }
}

impl<T> PoolGuard<T>
    where
        T: Clone,
{
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner,
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Acquires the context by adding 1 to the count
    #[inline]
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn acquire(&self) {
        let count = self.count.fetch_add(1, Ordering::Relaxed);
        tracing::trace!(count);
    }

    /// Releases the connection by subtracting from the stored count
    #[inline]
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn release(&self) {
        let count = self.count.fetch_sub(1, Ordering::Relaxed);
        tracing::trace!(count);
    }

    #[inline]
    pub(crate) fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

impl PooledContext {
    /// Creates a new pooled context from a list of contexts
    pub(crate) fn new(contexts: Vec<Context>) -> Self {
        Self {
            contexts: contexts.into_iter().map(PoolGuard::new).collect(),
        }
    }

    /// Acquires a context from the pool
    /// It always chooses the one that is used the least
    #[inline]
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn acquire(&self) -> PoolGuard<Context> {
        self.contexts
            .iter()
            .min_by_key(|c| c.count())
            .unwrap()
            .clone()
    }
}
