use crate::context::Context;
use crate::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event_handler::EventHandler;
use crate::Event;
use std::sync::Arc;
use tokio::net::tcp::OwnedReadHalf;

pub mod builder;
pub mod client;
pub mod context;
pub mod server;
pub mod stream_emitter;

/// Handles listening to a connection and triggering the corresponding event functions
async fn handle_connection(handler: Arc<EventHandler>, mut read_half: OwnedReadHalf, ctx: Context) {
    while let Ok(event) = Event::from_async_read(&mut read_half).await {
        // check if the event is a reply
        if let Some(ref_id) = event.reference_id() {
            // get the listener for replies
            if let Some(sender) = ctx.get_reply_sender(ref_id).await {
                // try sending the event to the listener for replies
                if let Err(event) = sender.send(event) {
                    handle_event(Context::clone(&ctx), Arc::clone(&handler), event);
                }
                continue;
            }
        }
        handle_event(Context::clone(&ctx), Arc::clone(&handler), event);
    }
    log::debug!("Connection closed.");
}

/// Handles a single event in a different tokio context
fn handle_event(ctx: Context, handler: Arc<EventHandler>, event: Event) {
    tokio::spawn(async move {
        if let Err(e) = handler.handle_event(&ctx, event).await {
            // emit an error event
            if let Err(e) = ctx
                .emitter
                .emit(
                    ERROR_EVENT_NAME,
                    ErrorEventData {
                        message: format!("{:?}", e),
                        code: 500,
                    },
                )
                .await
            {
                log::error!("Error occurred when sending error response: {:?}", e);
            }
            log::error!("Failed to handle event: {:?}", e);
        }
    });
}
