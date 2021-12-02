use crate::error::Result;
use crate::events::event::Event;
use crate::ipc::context::Context;
use crate::protocol::AsyncProtocolStream;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

type EventCallback<P> = Arc<
    dyn for<'a> Fn(&'a Context<P>, Event) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
        + Send
        + Sync,
>;

/// Handler for events
pub struct EventHandler<P: AsyncProtocolStream> {
    callbacks: HashMap<String, EventCallback<P>>,
}

impl<S> Clone for EventHandler<S>
where
    S: AsyncProtocolStream,
{
    fn clone(&self) -> Self {
        Self {
            callbacks: self.callbacks.clone(),
        }
    }
}

impl<P> Debug for EventHandler<P>
where
    P: AsyncProtocolStream,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let callback_names: String = self
            .callbacks
            .keys()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ");
        format!("EventHandler {{callbacks: [{}]}}", callback_names).fmt(f)
    }
}

impl<P> EventHandler<P>
where
    P: AsyncProtocolStream,
{
    /// Creates a new event handler
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
        }
    }

    /// Adds a new event callback
    #[tracing::instrument(skip(self, callback))]
    pub fn on<F: 'static>(&mut self, name: &str, callback: F)
    where
        F: for<'a> Fn(
                &'a Context<P>,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.callbacks.insert(name.to_string(), Arc::new(callback));
    }

    /// Handles a received event
    #[tracing::instrument(level = "debug", skip(self, ctx, event))]
    pub async fn handle_event(&self, ctx: &Context<P>, event: Event) -> Result<()> {
        if let Some(cb) = self.callbacks.get(event.name()) {
            cb.as_ref()(ctx, event).await?;
        }

        Ok(())
    }
}
