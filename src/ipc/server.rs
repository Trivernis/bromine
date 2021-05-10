use crate::error::Result;
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

pub struct IPCServer {
    pub(crate) handler: EventHandler,
}

impl IPCServer {
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
    pub async fn handle_connection(handler: Arc<EventHandler>, stream: TcpStream) {
        let (mut read_half, write_half) = stream.into_split();
        let emitter = StreamEmitter::new(write_half);
        let ctx = Context::new(StreamEmitter::clone(&emitter));

        while let Ok(event) = Event::from_async_read(&mut read_half).await {
            if let Err(e) = handler.handle_event(&ctx, event).await {
                log::error!("Failed to handle event: {:?}", e);
            }
        }
        log::debug!("Connection closed.");
    }
}
