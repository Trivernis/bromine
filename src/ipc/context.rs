use crate::ipc::stream_emitter::StreamEmitter;

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
}

impl Context {
    pub(crate) fn new(emitter: StreamEmitter) -> Self {
        Self { emitter }
    }
}
