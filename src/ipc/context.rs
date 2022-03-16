use std::collections::HashMap;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::Duration;
use trait_bound_typemap::SendSyncTypeMap;

use crate::error::{Error, Result};
use crate::event::{Event, EventType};
use crate::ipc::stream_emitter::emit_metadata::EmitMetadata;
use crate::ipc::stream_emitter::StreamEmitter;
use crate::payload::IntoPayload;
#[cfg(feature = "serialize")]
use crate::payload::{DynamicSerializer, SerdePayload};
use crate::prelude::Response;

pub(crate) type ReplyListeners = Arc<Mutex<HashMap<u64, mpsc::Sender<Event>>>>;

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
    pub data: Arc<RwLock<SendSyncTypeMap>>,

    stop_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,

    pub(crate) reply_listeners: ReplyListeners,

    pub default_reply_timeout: Duration,

    ref_id: Option<u64>,

    #[cfg(feature = "serialize")]
    pub default_serializer: DynamicSerializer,
}

impl Context {
    pub(crate) fn new(
        emitter: StreamEmitter,
        data: Arc<RwLock<SendSyncTypeMap>>,
        stop_sender: Option<oneshot::Sender<()>>,
        reply_listeners: ReplyListeners,
        reply_timeout: Duration,
        #[cfg(feature = "serialize")] default_serializer: DynamicSerializer,
    ) -> Self {
        Self {
            emitter,
            reply_listeners,
            data,
            stop_sender: Arc::new(Mutex::new(stop_sender)),
            default_reply_timeout: reply_timeout,
            #[cfg(feature = "serialize")]
            default_serializer,
            ref_id: None,
        }
    }

    /// Emits a raw event. Only for internal use
    pub(crate) fn emit_raw<P: IntoPayload>(
        &self,
        name: &str,
        namespace: Option<String>,
        event_type: EventType,
        payload: P,
    ) -> EmitMetadata<P> {
        self.emitter.emit_raw(
            self.clone(),
            self.ref_id.clone(),
            name,
            namespace,
            event_type,
            payload,
        )
    }

    /// Emits an event with a given payload that can be serialized into bytes
    pub fn emit<S: AsRef<str>, P: IntoPayload>(&self, name: S, payload: P) -> EmitMetadata<P> {
        if let Some(ref_id) = &self.ref_id {
            self.emitter
                .emit_response(self.clone(), *ref_id, name, payload)
        } else {
            self.emitter.emit(self.clone(), name, payload)
        }
    }

    /// Emits an event to a specific namespace
    pub fn emit_to<S1: AsRef<str>, S2: AsRef<str>, P: IntoPayload>(
        &self,
        namespace: S1,
        name: S2,
        payload: P,
    ) -> EmitMetadata<P> {
        if let Some(ref_id) = &self.ref_id {
            self.emitter
                .emit_response_to(self.clone(), *ref_id, namespace, name, payload)
        } else {
            self.emitter.emit_to(self.clone(), namespace, name, payload)
        }
    }

    /// Ends the event flow by creating a final response
    pub fn response<P: IntoPayload>(&self, payload: P) -> Result<Response> {
        Response::payload(self, payload)
    }

    /// Registers a reply listener for a given event
    #[inline]
    #[tracing::instrument(level = "debug", skip(self))]
    pub(crate) async fn register_reply_listener(&self, event_id: u64) -> Result<Receiver<Event>> {
        let (rx, tx) = mpsc::channel(8);
        {
            let mut listeners = self.reply_listeners.lock().await;
            listeners.insert(event_id, rx);
        }

        Ok(tx)
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
    pub(crate) async fn get_reply_sender(&self, ref_id: u64) -> Option<mpsc::Sender<Event>> {
        let listeners = self.reply_listeners.lock().await;
        listeners.get(&ref_id).cloned()
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
