use super::handle_connection;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use std::sync::Arc;
use tokio::net::TcpStream;

/// The IPC Client to connect to an IPC Server.
/// Use the [IPCBuilder](crate::builder::IPCBuilder) to create the client.
/// Usually one does not need to use the IPCClient object directly.
pub struct IPCClient {
    pub(crate) handler: EventHandler,
}

impl IPCClient {
    /// Connects to a given address and returns an emitter for events to that address.
    /// Invoked by [IPCBuilder::build_client](crate::builder::IPCBuilder::build_client)
    pub async fn connect(self, address: &str) -> Result<Context> {
        let stream = TcpStream::connect(address).await?;
        let (read_half, write_half) = stream.into_split();
        let emitter = StreamEmitter::new(write_half);
        let ctx = Context::new(StreamEmitter::clone(&emitter));
        let handler = Arc::new(self.handler);

        tokio::spawn({
            let ctx = Context::clone(&ctx);
            async move {
                handle_connection(handler, read_half, ctx).await;
            }
        });

        Ok(ctx)
    }
}
