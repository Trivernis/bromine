use crate::error::{Error, Result};
use crate::event::Event;
use crate::ipc::stream_emitter::StreamEmitter;
use crate::protocol::AsyncProtocolStream;
use std::collections::HashMap;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex, RwLock};
use typemap_rev::TypeMap;

pub(crate) type ReplyListeners = Arc<Mutex<HashMap<u64, oneshot::Sender<Event>>>>;

/// An object provided to each callback function.
/// Currently it only holds the event emitter to emit response events in event callbacks.
/// ```rust
/// use rmp_ipc::prelude::*;
///
/// async fn my_callback<S: AsyncProtocolStream>(ctx: &Context<S>, _event: Event) -> IPCResult<()> {
///     // use the emitter on the context object to emit events
///     // inside callbacks
///     ctx.emitter.emit("ping", ()).await?;
///     Ok(())
/// }
/// ```
pub struct Context<S: AsyncProtocolStream> {
    /// The event emitter
    pub emitter: StreamEmitter<S>,

    /// Field to store additional context data
    pub data: Arc<RwLock<TypeMap>>,

    stop_sender: Arc<Mutex<Option<Sender<()>>>>,

    reply_listeners: ReplyListeners,
}

impl<S> Clone for Context<S>
where
    S: AsyncProtocolStream,
{
    fn clone(&self) -> Self {
        Self {
            emitter: self.emitter.clone(),
            data: Arc::clone(&self.data),
            stop_sender: Arc::clone(&self.stop_sender),
            reply_listeners: Arc::clone(&self.reply_listeners),
        }
    }
}

impl<P> Context<P>
where
    P: AsyncProtocolStream,
{
    pub(crate) fn new(
        emitter: StreamEmitter<P>,
        data: Arc<RwLock<TypeMap>>,
        stop_sender: Option<Sender<()>>,
        reply_listeners: ReplyListeners,
    ) -> Self {
        Self {
            emitter,
            reply_listeners,
            data,
            stop_sender: Arc::new(Mutex::new(stop_sender)),
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
        let event = tx.await?;

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

    /// Returns the channel for a reply to the given message id
    pub(crate) async fn get_reply_sender(&self, ref_id: u64) -> Option<oneshot::Sender<Event>> {
        let mut listeners = self.reply_listeners.lock().await;
        listeners.remove(&ref_id)
    }
}

pub struct PooledContext<S: AsyncProtocolStream> {
    contexts: Vec<PoolGuard<Context<S>>>,
}

impl<S> Clone for PooledContext<S>
where
    S: AsyncProtocolStream,
{
    fn clone(&self) -> Self {
        Self {
            contexts: self.contexts.clone(),
        }
    }
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

impl<P> PooledContext<P>
where
    P: AsyncProtocolStream,
{
    /// Creates a new pooled context from a list of contexts
    pub(crate) fn new(contexts: Vec<Context<P>>) -> Self {
        Self {
            contexts: contexts.into_iter().map(PoolGuard::new).collect(),
        }
    }

    /// Acquires a context from the pool
    /// It always chooses the one that is used the least
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn acquire(&self) -> PoolGuard<Context<P>> {
        self.contexts
            .iter()
            .min_by_key(|c| c.count())
            .unwrap()
            .clone()
    }
}
