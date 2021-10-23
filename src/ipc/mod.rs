use crate::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event_handler::EventHandler;
use crate::namespaces::namespace::Namespace;
use crate::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::tcp::OwnedReadHalf;

pub mod builder;
pub mod client;
pub mod context;
pub mod server;
pub mod stream_emitter;

/// Handles listening to a connection and triggering the corresponding event functions
async fn handle_connection(
    namespaces: Arc<HashMap<String, Namespace>>,
    handler: Arc<EventHandler>,
    mut read_half: OwnedReadHalf,
    ctx: Context,
) {
    while let Ok(event) = Event::from_async_read(&mut read_half).await {
        tracing::trace!("event = {:?}", event);
        // check if the event is a reply
        if let Some(ref_id) = event.reference_id() {
            tracing::trace!("Event has reference id. Passing to reply listener");
            // get the listener for replies
            if let Some(sender) = ctx.get_reply_sender(ref_id).await {
                // try sending the event to the listener for replies
                if let Err(event) = sender.send(event) {
                    handle_event(Context::clone(&ctx), Arc::clone(&handler), event);
                }
                continue;
            }
            tracing::trace!("No response listener found for event. Passing to regular listener.");
        }
        if let Some(namespace) = event.namespace().clone().and_then(|n| namespaces.get(&n)) {
            tracing::trace!("Passing event to namespace listener");
            let handler = Arc::clone(&namespace.handler);
            handle_event(Context::clone(&ctx), handler, event);
        } else {
            tracing::trace!("Passing event to global listener");
            handle_event(Context::clone(&ctx), Arc::clone(&handler), event);
        }
    }
    tracing::debug!("Connection closed.");
}

/// Handles a single event in a different tokio context
fn handle_event(ctx: Context, handler: Arc<EventHandler>, event: Event) {
    tokio::spawn(async move {
        let id = event.id();
        if let Err(e) = handler.handle_event(&ctx, event).await {
            // emit an error event
            if let Err(e) = ctx
                .emitter
                .emit_response(
                    id,
                    ERROR_EVENT_NAME,
                    ErrorEventData {
                        message: format!("{:?}", e),
                        code: 500,
                    },
                )
                .await
            {
                tracing::error!("Error occurred when sending error response: {:?}", e);
            }
            tracing::error!("Failed to handle event: {:?}", e);
        }
    });
}