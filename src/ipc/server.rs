use super::handle_connection;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use crate::namespaces::namespace::Namespace;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use typemap_rev::TypeMap;

/// The IPC Server listening for connections.
/// Use the [IPCBuilder](crate::builder::IPCBuilder) to create a server.
/// Usually one does not need to use the IPCServer object directly.
pub struct IPCServer {
    pub(crate) handler: EventHandler,
    pub(crate) namespaces: HashMap<String, Namespace>,
    pub(crate) data: TypeMap,
}

impl IPCServer {
    /// Starts the IPC Server.
    /// Invoked by [IPCBuilder::build_server](crate::builder::IPCBuilder::build_server)
    #[tracing::instrument(skip(self))]
    pub async fn start(self, address: &str) -> Result<()> {
        let listener = TcpListener::bind(address).await?;
        let handler = Arc::new(self.handler);
        let namespaces = Arc::new(self.namespaces);
        let data = Arc::new(RwLock::new(self.data));
        tracing::info!(address);

        while let Ok((stream, remote_address)) = listener.accept().await {
            let remote_address = remote_address.to_string();
            tracing::debug!("remote_address = {}", remote_address);
            let handler = Arc::clone(&handler);
            let namespaces = Arc::clone(&namespaces);
            let data = Arc::clone(&data);

            tokio::spawn(async {
                let (read_half, write_half) = stream.into_split();
                let emitter = StreamEmitter::new(write_half);
                let ctx = Context::new(StreamEmitter::clone(&emitter), data, None);

                handle_connection(namespaces, handler, read_half, ctx).await;
            });
        }

        Ok(())
    }
}
