use crate::error::Result;
use crate::events::event::Event;
use crate::ipc::context::Context;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
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

impl Debug for EventHandler {
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

impl EventHandler {
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
                &'a Context,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.callbacks.insert(name.to_string(), Arc::new(callback));
    }

    /// Handles a received event
    #[tracing::instrument(level = "debug", skip(self, ctx, event))]
    pub async fn handle_event(&self, ctx: &Context, event: Event) -> Result<()> {
        if let Some(cb) = self.callbacks.get(event.name()) {
            cb.as_ref()(ctx, event).await?;
        }

        Ok(())
    }
}
