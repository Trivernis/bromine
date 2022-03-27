use crate::error::{Error, Result};
use crate::error_event::ErrorEventData;
use crate::event_handler::Response;
use crate::events::error_event::ERROR_EVENT_NAME;
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::client::IPCClient;
use crate::ipc::context::{Context, PooledContext, ReplyListeners};
use crate::ipc::server::IPCServer;
use crate::namespaces::builder::NamespaceBuilder;
use crate::namespaces::namespace::Namespace;
#[cfg(feature = "serialize")]
use crate::payload::DynamicSerializer;
use crate::prelude::AsyncProtocolStream;
use crate::protocol::AsyncStreamProtocolListener;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use trait_bound_typemap::{KeyCanExtend, SendSyncTypeMap, TypeMap, TypeMapEntry, TypeMapKey};

/// A builder for the IPC server or client.
/// ```no_run
/// use std::net::ToSocketAddrs;
/// use trait_bound_typemap::TypeMapKey;
/// use bromine::IPCBuilder;
/// use tokio::net::TcpListener;
/// use bromine::prelude::Response;
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
///         Ok(Response::empty())
///     }))
///     // register a namespace    
///     .namespace("namespace")
///     .on("namespace-event", |_ctx, _event| Box::pin(async move {
///         println!("Namespace event.");
///         Ok(Response::empty())
///     }))
///     .build()
///     // add context shared data
///     .insert::<CustomKey>("Hello World".to_string())
///     // can also be build_client which would return an emitter for events
///     .build_server().await.unwrap();
///# }
/// ```
///
pub struct IPCBuilder<L: AsyncStreamProtocolListener> {
    handler: EventHandler,
    address: Option<L::AddressType>,
    namespaces: HashMap<String, Namespace>,
    data: SendSyncTypeMap,
    timeout: Duration,
    #[cfg(feature = "serialize")]
    default_serializer: DynamicSerializer,
    listener_options: L::ListenerOptions,
    stream_options: <L::Stream as AsyncProtocolStream>::StreamOptions,
}

impl<L> IPCBuilder<L>
where
    L: AsyncStreamProtocolListener,
{
    pub fn new() -> Self {
        let mut handler = EventHandler::new();
        handler.on(ERROR_EVENT_NAME, |_, event| {
            Box::pin(async move {
                let error_data = event.payload::<ErrorEventData>()?;
                tracing::warn!(error_data.code);
                tracing::warn!("error_data.message = '{}'", error_data.message);

                Ok(Response::empty())
            })
        });
        Self {
            handler,
            address: None,
            namespaces: HashMap::new(),
            data: SendSyncTypeMap::new(),
            timeout: Duration::from_secs(60),
            #[cfg(feature = "serialize")]
            default_serializer: DynamicSerializer::first_available(),
            listener_options: Default::default(),
            stream_options: Default::default(),
        }
    }

    /// Adds globally shared data
    pub fn insert<K: TypeMapKey>(mut self, value: K::Value) -> Self
    where
        <K as TypeMapKey>::Value: Send + Sync,
    {
        self.data.insert::<K>(value);

        self
    }

    /// Adds all the data from the other given type map
    pub fn insert_all<I: IntoIterator<Item = TypeMapEntry<K>>, K: KeyCanExtend<SendSyncTypeMap>>(
        mut self,
        value: I,
    ) -> Self {
        self.data.extend(value);

        self
    }

    /// Adds an event callback
    pub fn on<F: 'static>(mut self, event: &str, callback: F) -> Self
    where
        F: for<'a> Fn(
                &'a Context,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<Response>> + Send + 'a)>>
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
    pub fn add_namespace(mut self, namespace: Namespace) -> Self {
        self.namespaces
            .insert(namespace.name().to_owned(), namespace);

        self
    }

    /// Sets the timeout when listening for a response
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;

        self
    }

    #[cfg(feature = "serialize")]
    /// Sets the default serializer used for rust types that implement
    /// serdes Serialize or Deserialize
    pub fn default_serializer(mut self, serializer: DynamicSerializer) -> Self {
        self.default_serializer = serializer;

        self
    }

    /// Sets the options for the given protocol listener
    pub fn server_options(mut self, options: L::ListenerOptions) -> Self {
        self.listener_options = options;

        self
    }

    /// Sets the options for the given protocol stream
    pub fn client_options(
        mut self,
        options: <L::Stream as AsyncProtocolStream>::StreamOptions,
    ) -> Self {
        self.stream_options = options;

        self
    }

    /// Builds an ipc server
    #[tracing::instrument(skip(self))]
    pub async fn build_server(self) -> Result<()> {
        self.validate()?;
        let server = IPCServer {
            namespaces: self.namespaces,
            handler: self.handler,
            data: self.data,
            timeout: self.timeout,

            #[cfg(feature = "serialize")]
            default_serializer: self.default_serializer,
        };
        server
            .start::<L>(self.address.unwrap(), self.listener_options)
            .await?;

        Ok(())
    }

    /// Builds an ipc client
    #[tracing::instrument(skip(self))]
    pub async fn build_client(self) -> Result<Context> {
        self.validate()?;
        let data = Arc::new(RwLock::new(self.data));
        let reply_listeners = ReplyListeners::default();

        let client = IPCClient {
            namespaces: self.namespaces,
            handler: self.handler,
            data,
            reply_listeners,
            timeout: self.timeout,
            #[cfg(feature = "serialize")]
            default_serializer: self.default_serializer,
        };

        let ctx = client
            .connect::<L::Stream>(self.address.unwrap(), self.stream_options.clone())
            .await?;

        Ok(ctx)
    }

    /// Builds a pooled IPC client
    /// This causes the builder to actually create `pool_size` clients and
    /// return a [crate::context::PooledContext] that allows one to [crate::context::PooledContext::acquire] a single context
    /// to emit events.
    #[tracing::instrument(skip(self))]
    pub async fn build_pooled_client(self, pool_size: usize) -> Result<PooledContext> {
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
                reply_listeners: reply_listeners.clone(),
                timeout: self.timeout.clone(),

                #[cfg(feature = "serialize")]
                default_serializer: self.default_serializer.clone(),
            };

            let ctx = client
                .connect::<L::Stream>(address.clone(), self.stream_options.clone())
                .await?;
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
