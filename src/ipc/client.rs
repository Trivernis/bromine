use super::handle_connection;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::{Context, ReplyListeners};
use crate::ipc::stream_emitter::StreamEmitter;
use crate::namespaces::namespace::Namespace;
use crate::protocol::AsyncProtocolStream;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::RwLock;
use trait_bound_typemap::SendSyncTypeMap;

#[cfg(feature = "serialize")]
use crate::payload::DynamicSerializer;

/// The IPC Client to connect to an IPC Server.
/// Use the [IPCBuilder](crate::builder::IPCBuilder) to create the client.
/// Usually one does not need to use the IPCClient object directly.
#[derive(Clone)]
pub struct IPCClient {
    pub(crate) handler: EventHandler,
    pub(crate) namespaces: HashMap<String, Namespace>,
    pub(crate) data: Arc<RwLock<SendSyncTypeMap>>,
    pub(crate) reply_listeners: ReplyListeners,
    pub(crate) timeout: Duration,

    #[cfg(feature = "serialize")]
    pub(crate) default_serializer: DynamicSerializer,
}

impl IPCClient {
    /// Connects to a given address and returns an emitter for events to that address.
    /// Invoked by [IPCBuilder::build_client](crate::builder::IPCBuilder::build_client)
    #[tracing::instrument(skip(self, options))]
    pub async fn connect<S: AsyncProtocolStream + 'static>(
        self,
        address: S::AddressType,
        options: S::StreamOptions,
    ) -> Result<Context> {
        let stream = S::protocol_connect(address, options).await?;
        let (read_half, write_half) = stream.protocol_into_split();

        let emitter = StreamEmitter::new::<S>(write_half);

        let (tx, rx) = oneshot::channel();
        #[cfg(feature = "serialize")]
        let ctx = Context::new(
            StreamEmitter::clone(&emitter),
            self.data,
            Some(tx),
            self.reply_listeners,
            self.timeout,
            self.default_serializer,
        );
        #[cfg(not(feature = "serialize"))]
        let ctx = Context::new(
            StreamEmitter::clone(&emitter),
            self.data,
            Some(tx),
            self.reply_listeners,
            self.timeout,
        );
        let handler = Arc::new(self.handler);
        let namespaces = Arc::new(self.namespaces);

        let handle = tokio::spawn({
            let ctx = Context::clone(&ctx);
            async move {
                handle_connection::<S>(namespaces, handler, read_half, ctx).await;
            }
        });
        tokio::spawn(async move {
            let _ = rx.await;
            handle.abort();
        });

        Ok(ctx)
    }
}
