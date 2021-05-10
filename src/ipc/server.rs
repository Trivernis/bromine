use crate::error::Result;
use crate::events::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

/// The IPC Server listening for connections.
/// Use the [IPCBuilder](crate::builder::IPCBuilder) to create a server.
/// Usually one does not need to use the IPCServer object directly.
pub struct IPCServer {
    pub(crate) handler: EventHandler,
}

impl IPCServer {
    /// Starts the IPC Server.
    /// Invoked by [IPCBuilder::build_server](crate::builder::IPCBuilder::build_server)
    pub async fn start(self, address: &str) -> Result<()> {
        let listener = TcpListener::bind(address).await?;
        let handler = Arc::new(self.handler);

        while let Ok((stream, _)) = listener.accept().await {
            let handler = handler.clone();

            tokio::spawn(async {
                Self::handle_connection(handler, stream).await;
            });
        }

        Ok(())
    }

    /// Handles a single tcp connection
    async fn handle_connection(handler: Arc<EventHandler>, stream: TcpStream) {
        let (mut read_half, write_half) = stream.into_split();
        let emitter = StreamEmitter::new(write_half);
        let ctx = Context::new(StreamEmitter::clone(&emitter));

        while let Ok(event) = Event::from_async_read(&mut read_half).await {
            if let Err(e) = handler.handle_event(&ctx, event).await {
                // emit an error event
                if emitter
                    .emit(
                        ERROR_EVENT_NAME,
                        ErrorEventData {
                            message: format!("{:?}", e),
                            code: 500,
                        },
                    )
                    .await
                    .is_err()
                {
                    break;
                }
                log::error!("Failed to handle event: {:?}", e);
            }
        }
        log::debug!("Connection closed.");
    }
}
