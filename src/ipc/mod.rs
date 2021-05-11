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
        let ctx = Context::clone(&ctx);
        let handler = Arc::clone(&handler);

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
    log::debug!("Connection closed.");
}
