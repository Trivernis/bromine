use crate::error::{Error, Result};
use crate::events::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::client::IPCClient;
use crate::ipc::context::{Context, PooledContext, ReplyListeners};
use crate::ipc::server::IPCServer;
use crate::namespaces::builder::NamespaceBuilder;
use crate::namespaces::namespace::Namespace;
use crate::protocol::AsyncStreamProtocolListener;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use typemap_rev::{TypeMap, TypeMapKey};

/// A builder for the IPC server or client.
/// ```no_run
/// use std::net::ToSocketAddrs;
/// use typemap_rev::TypeMapKey;
/// use bromine::IPCBuilder;
/// use tokio::net::TcpListener;
///
/// struct CustomKey;
///
/// impl TypeMapKey for CustomKey {
///     type Value = String;
/// }
///
///# async fn a() {
/// IPCBuilder::<TcpListener>::new()
///     .address("127.0.0.1:2020".to_socket_addrs().unwrap().next().unwrap())
///    // register callback
///     .on("ping", |_ctx, _event| Box::pin(async move {
///         println!("Received ping event.");
///         Ok(())
///     }))
///     // register a namespace    
///     .namespace("namespace")
///     .on("namespace-event", |_ctx, _event| Box::pin(async move {
///         println!("Namespace event.");
///         Ok(())
///     }))
///     .build()
///     // add context shared data
///     .insert::<CustomKey>("Hello World".to_string())
///     // can also be build_client which would return an emitter for events
///     .build_server().await.unwrap();
///# }
/// ```
pub struct IPCBuilder<L: AsyncStreamProtocolListener> {
    handler: EventHandler<L::Stream>,
    address: Option<L::AddressType>,
    namespaces: HashMap<String, Namespace<L::Stream>>,
    data: TypeMap,
    timeout: Duration,
}

impl<L> IPCBuilder<L>
where
    L: AsyncStreamProtocolListener,
{
    pub fn new() -> Self {
        let mut handler = EventHandler::new();
        handler.on(ERROR_EVENT_NAME, |_, event| {
            Box::pin(async move {
                let error_data = event.data::<ErrorEventData>()?;
                tracing::warn!(error_data.code);
                tracing::warn!("error_data.message = '{}'", error_data.message);

                Ok(())
            })
        });
        Self {
            handler,
            address: None,
            namespaces: HashMap::new(),
            data: TypeMap::new(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Adds globally shared data
    pub fn insert<K: TypeMapKey>(mut self, value: K::Value) -> Self {
        self.data.insert::<K>(value);

        self
    }

    /// Adds an event callback
    pub fn on<F: 'static>(mut self, event: &str, callback: F) -> Self
    where
        F: for<'a> Fn(
                &'a Context<L::Stream>,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.handler.on(event, callback);

        self
    }

    /// Adds the address to connect to
    pub fn address(mut self, address: L::AddressType) -> Self {
        self.address = Some(address);

        self
    }

    /// Adds a namespace
    pub fn namespace<S: ToString>(self, name: S) -> NamespaceBuilder<L> {
        NamespaceBuilder::new(self, name.to_string())
    }

    /// Adds a namespace to the ipc server
    pub fn add_namespace(mut self, namespace: Namespace<L::Stream>) -> Self {
        self.namespaces
            .insert(namespace.name().to_owned(), namespace);

        self
    }

    /// Sets the timeout when listening for a response
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;

        self
    }

    /// Builds an ipc server
    #[tracing::instrument(skip(self))]
    pub async fn build_server(self) -> Result<()> {
        self.validate()?;
        let server = IPCServer::<L> {
            namespaces: self.namespaces,
            handler: self.handler,
            data: self.data,
            timeout: self.timeout,
        };
        server.start(self.address.unwrap()).await?;

        Ok(())
    }

    /// Builds an ipc client
    #[tracing::instrument(skip(self))]
    pub async fn build_client(self) -> Result<Context<L::Stream>> {
        self.validate()?;
        let data = Arc::new(RwLock::new(self.data));
        let reply_listeners = ReplyListeners::default();
        let client = IPCClient {
            namespaces: self.namespaces,
            handler: self.handler,
            data,
            reply_listeners,
            timeout: self.timeout,
        };

        let ctx = client.connect(self.address.unwrap()).await?;

        Ok(ctx)
    }

    /// Builds a pooled IPC client
    /// This causes the builder to actually create `pool_size` clients and
    /// return a [crate::context::PooledContext] that allows one to [crate::context::PooledContext::acquire] a single context
    /// to emit events.
    #[tracing::instrument(skip(self))]
    pub async fn build_pooled_client(self, pool_size: usize) -> Result<PooledContext<L::Stream>> {
        if pool_size == 0 {
            Error::BuildError("Pool size must be greater than 0".to_string());
        }
        self.validate()?;
        let data = Arc::new(RwLock::new(self.data));
        let mut contexts = Vec::new();
        let address = self.address.unwrap();
        let reply_listeners = ReplyListeners::default();

        for _ in 0..pool_size {
            let client = IPCClient {
                namespaces: self.namespaces.clone(),
                handler: self.handler.clone(),
                data: Arc::clone(&data),
                reply_listeners: Arc::clone(&reply_listeners),
                timeout: self.timeout.clone(),
            };

            let ctx = client.connect(address.clone()).await?;
            contexts.push(ctx);
        }

        Ok(PooledContext::new(contexts))
    }

    /// Validates that all required fields have been provided
    #[tracing::instrument(skip(self))]
    fn validate(&self) -> Result<()> {
        if self.address.is_none() {
            Err(Error::BuildError("Missing Address".to_string()))
        } else {
            Ok(())
        }
    }
}
