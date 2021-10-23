use crate::error::{Error, Result};
use crate::events::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::client::IPCClient;
use crate::ipc::context::Context;
use crate::ipc::server::IPCServer;
use crate::namespaces::builder::NamespaceBuilder;
use crate::namespaces::namespace::Namespace;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use typemap_rev::{TypeMap, TypeMapKey};

/// A builder for the IPC server or client.
/// ```no_run
///use typemap_rev::TypeMapKey;
/// use rmp_ipc::IPCBuilder;
///
/// struct CustomKey;
///
/// impl TypeMapKey for CustomKey {
///     type Value = String;
/// }
///
///# async fn a() {
/// IPCBuilder::new()
///     .address("127.0.0.1:2020")
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
pub struct IPCBuilder {
    handler: EventHandler,
    address: Option<String>,
    namespaces: HashMap<String, Namespace>,
    data: TypeMap,
}

impl IPCBuilder {
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
                &'a Context,
                Event,
            ) -> Pin<Box<(dyn Future<Output = Result<()>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.handler.on(event, callback);

        self
    }

    /// Adds the address to connect to
    pub fn address<S: ToString>(mut self, address: S) -> Self {
        self.address = Some(address.to_string());

        self
    }

    /// Adds a namespace
    pub fn namespace<S: ToString>(self, name: S) -> NamespaceBuilder {
        NamespaceBuilder::new(self, name.to_string())
    }

    /// Adds a namespace to the ipc server
    pub fn add_namespace(mut self, namespace: Namespace) -> Self {
        self.namespaces
            .insert(namespace.name().to_owned(), namespace);

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
        };
        server.start(&self.address.unwrap()).await?;

        Ok(())
    }

    /// Builds an ipc client
    #[tracing::instrument(skip(self))]
    pub async fn build_client(self) -> Result<Context> {
        self.validate()?;
        let client = IPCClient {
            namespaces: self.namespaces,
            handler: self.handler,
            data: self.data,
        };

        let ctx = client.connect(&self.address.unwrap()).await?;

        Ok(ctx)
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
