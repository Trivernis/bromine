use crate::error::Result;
use crate::events::event::Event;
use crate::ipc::context::Context;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

type EventCallback = Arc<
    dyn for<'a> Fn(&'a Context, Event) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
        + Send
        + Sync,
>;

/// Handler for events
#[derive(Clone)]
pub struct EventHandler {
    callbacks: HashMap<String, EventCallback>,
}

impl EventHandler {
    /// Creates a new event handler
    pub fn new() -> Self {
        Self {
            callbacks: HashMap::new(),
        }
    }

    /// Adds a new event callback
    pub fn on<F: 'static>(&mut self, name: &str, callback: F)
    where
        F: for<'a> Fn(
                &'a Context,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.callbacks.insert(name.to_string(), Arc::new(callback));
    }

    /// Handles a received event
    pub async fn handle_event(&self, ctx: &Context, event: Event) -> Result<()> {
        if let Some(cb) = self.callbacks.get(event.name()) {
            cb.as_ref()(ctx, event).await?;
        }

        Ok(())
    }
}
