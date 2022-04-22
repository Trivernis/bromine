use crate::error::Result;
use crate::events::event::Event;
use crate::ipc::context::Context;
use crate::payload::{BytePayload, IntoPayload};
use bytes::Bytes;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct Response(Bytes);

impl Response {
    /// Creates a new response with a given payload
    pub fn payload<P: IntoPayload>(ctx: &Context, payload: P) -> Result<Self> {
        let bytes = payload.into_payload(ctx)?;

        Ok(Self(bytes))
    }

    /// Creates an empty response
    pub fn empty() -> Self {
        Self(Bytes::new())
    }

    pub(crate) fn into_byte_payload(self) -> BytePayload {
        BytePayload::from(self.0)
    }
}

type EventCallback = Arc<
    dyn for<'a> Fn(
            &'a Context,
            Event,
        ) -> Pin<Box<(dyn Future<Output = Result<Response>> + Send + 'a)>>
        + Send
        + Sync,
>;

/// Handler for events
#[derive(Clone, Default)]
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
        Self::default()
    }

    /// Adds a new event callback
    #[tracing::instrument(skip(self, callback))]
    pub fn on<F: 'static>(&mut self, name: &str, callback: F)
    where
        F: for<'a> Fn(
                &'a Context,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<Response>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.callbacks.insert(name.to_string(), Arc::new(callback));
    }

    /// Handles a received event
    #[inline]
    #[tracing::instrument(level = "debug", skip(self, ctx, event))]
    pub async fn handle_event(&self, ctx: &Context, event: Event) -> Result<Response> {
        if let Some(cb) = self.callbacks.get(event.name()) {
            cb.as_ref()(ctx, event).await
        } else {
            Ok(Response::empty())
        }
    }
}
