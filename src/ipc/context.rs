use crate::error::Result;
use crate::ipc::stream_emitter::StreamEmitter;
use crate::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

/// An object provided to each callback function.
/// Currently it only holds the event emitter to emit response events in event callbacks.
/// ```rust
/// use rmp_ipc::context::Context;
/// use rmp_ipc::Event;
/// use rmp_ipc::error::Result;
///
/// async fn my_callback(ctx: &Context, _event: Event) -> Result<()> {
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

    reply_listeners: Arc<Mutex<HashMap<u64, oneshot::Sender<Event>>>>,
}

impl Context {
    pub(crate) fn new(emitter: StreamEmitter) -> Self {
        Self {
            emitter,
            reply_listeners: Arc::new(Mutex::new(HashMap::new())),
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

    /// Returns the channel for a reply to the given message id
    pub(crate) async fn get_reply_sender(&self, ref_id: u64) -> Option<oneshot::Sender<Event>> {
        let mut listeners = self.reply_listeners.lock().await;
        listeners.remove(&ref_id)
    }
}
