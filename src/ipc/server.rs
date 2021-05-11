use super::handle_connection;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use std::sync::Arc;
use tokio::net::TcpListener;

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
            let handler = Arc::clone(&handler);

            tokio::spawn(async {
                let (read_half, write_half) = stream.into_split();
                let emitter = StreamEmitter::new(write_half);
                let ctx = Context::new(StreamEmitter::clone(&emitter));

                handle_connection(handler, read_half, ctx).await;
            });
        }

        Ok(())
    }
}
