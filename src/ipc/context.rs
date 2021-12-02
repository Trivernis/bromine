use crate::error::{Error, Result};
use crate::event::Event;
use crate::ipc::stream_emitter::StreamEmitter;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex, RwLock};
use typemap_rev::TypeMap;

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

    reply_listeners: Arc<Mutex<HashMap<u64, oneshot::Sender<Event>>>>,
}

impl Context {
    pub(crate) fn new(
        emitter: StreamEmitter,
        data: Arc<RwLock<TypeMap>>,
        stop_sender: Option<Sender<()>>,
    ) -> Self {
        Self {
            emitter,
            reply_listeners: Arc::new(Mutex::new(HashMap::new())),
            data,
            stop_sender: Arc::new(Mutex::new(stop_sender)),
        }
    }

    /// Waits for a reply to the given message ID
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
