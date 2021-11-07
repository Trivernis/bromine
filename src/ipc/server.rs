use super::handle_connection;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::{Context, ReplyListeners};
use crate::ipc::stream_emitter::StreamEmitter;
use crate::namespaces::namespace::Namespace;
use crate::protocol::{AsyncProtocolStreamSplit, AsyncStreamProtocolListener};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use typemap_rev::TypeMap;

/// The IPC Server listening for connections.
/// Use the [IPCBuilder](crate::builder::IPCBuilder) to create a server.
/// Usually one does not need to use the IPCServer object directly.
pub struct IPCServer<L: AsyncStreamProtocolListener> {
    pub(crate) handler: EventHandler<L::Stream>,
    pub(crate) namespaces: HashMap<String, Namespace<L::Stream>>,
    pub(crate) data: TypeMap,
}

impl<L> IPCServer<L>
where
    L: AsyncStreamProtocolListener,
{
    /// Starts the IPC Server.
    /// Invoked by [IPCBuilder::build_server](crate::builder::IPCBuilder::build_server)
    #[tracing::instrument(skip(self))]
    pub async fn start(self, address: L::AddressType) -> Result<()> {
        let listener = L::protocol_bind(address.clone()).await?;
        let handler = Arc::new(self.handler);
        let namespaces = Arc::new(self.namespaces);
        let data = Arc::new(RwLock::new(self.data));
        tracing::info!("address = {:?}", address);

        while let Ok((stream, remote_address)) = listener.protocol_accept().await {
            tracing::debug!("remote_address = {:?}", remote_address);
            let handler = Arc::clone(&handler);
            let namespaces = Arc::clone(&namespaces);
            let data = Arc::clone(&data);

            tokio::spawn(async {
                let (read_half, write_half) = stream.protocol_into_split();
                let emitter = StreamEmitter::new(write_half);
                let reply_listeners = ReplyListeners::default();
                let ctx = Context::new(StreamEmitter::clone(&emitter), data, None, reply_listeners);

                handle_connection(namespaces, handler, read_half, ctx).await;
            });
        }

        Ok(())
    }
}
