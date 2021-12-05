use crate::error::{Error, Result};
use crate::event::Event;
use crate::ipc::stream_emitter::StreamEmitter;
use futures::future;
use futures::future::Either;
use std::collections::HashMap;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::time::Duration;
use typemap_rev::TypeMap;

#[cfg(feature = "serialize")]
use crate::payload::{DynamicSerializer, SerdePayload};

pub(crate) type ReplyListeners = Arc<Mutex<HashMap<u64, oneshot::Sender<Event>>>>;

/// An object provided to each callback function.
/// Currently it only holds the event emitter to emit response events in event callbacks.
/// ```rust
/// use bromine::prelude::*;
///
/// async fn my_callback(ctx: &Context, _event: Event) -> IPCResult<()> {
///     // use the emitter on the context object to emit events
///     // inside callbacks
///     ctx.emitter.emit("ping", ()).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Context {
    /// The event emitter
    pub emitter: StreamEmitter,

    /// Field to store additional context data
    pub data: Arc<RwLock<TypeMap>>,

    stop_sender: Arc<Mutex<Option<Sender<()>>>>,

    reply_listeners: ReplyListeners,

    reply_timeout: Duration,

    #[cfg(feature = "serialize")]
    default_serializer: DynamicSerializer,
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
        }
    }

    /// Waits for a reply to the given message ID
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn await_reply(&self, message_id: u64) -> Result<Event> {
        let (rx, tx) = oneshot::channel();
        {
            let mut listeners = self.reply_listeners.lock().await;
            listeners.insert(message_id, rx);
        }
        let result = future::select(
            Box::pin(tx),
            Box::pin(tokio::time::sleep(self.reply_timeout)),
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
    pub fn create_serde_payload<T>(&self, data: T) -> SerdePayload<T> {
        SerdePayload::new(self.default_serializer.clone(), data)
    }

    /// Returns the channel for a reply to the given message id
    pub(crate) async fn get_reply_sender(&self, ref_id: u64) -> Option<oneshot::Sender<Event>> {
        let mut listeners = self.reply_listeners.lock().await;
        listeners.remove(&ref_id)
    }
}

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

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for PoolGuard<T>
where
    T: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> Clone for PoolGuard<T>
where
    T: Clone,
{
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
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn acquire(&self) {
        let count = self.count.fetch_add(1, Ordering::Relaxed);
        tracing::trace!(count);
    }

    /// Releases the connection by subtracting from the stored count
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn release(&self) {
        let count = self.count.fetch_sub(1, Ordering::Relaxed);
        tracing::trace!(count);
    }

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
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn acquire(&self) -> PoolGuard<Context> {
        self.contexts
            .iter()
            .min_by_key(|c| c.count())
            .unwrap()
            .clone()
    }
}
