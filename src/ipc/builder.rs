use crate::error::{Error, Result};
use crate::events::error_event::{ErrorEventData, ERROR_EVENT_NAME};
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::client::IPCClient;
use crate::ipc::context::Context;
use crate::ipc::server::IPCServer;
use crate::ipc::stream_emitter::StreamEmitter;
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
/// A builder for the IPC server or client.
/// ```no_run
///use rmp_ipc::IPCBuilder;
///# async fn a() {
/// IPCBuilder::new()
///     .address("127.0.0.1:2020")
///    // register callback
///     .on("ping", |_ctx, _event| Box::pin(async move {
///         println!("Received ping event.");
///         Ok(())
///     }))
///     // can also be build_client which would return an emitter for events
///     .build_server().await.unwrap();
///# }
/// ```
pub struct IPCBuilder {
    handler: EventHandler,
    address: Option<String>,
}

impl IPCBuilder {
    pub fn new() -> Self {
        let mut handler = EventHandler::new();
        handler.on(ERROR_EVENT_NAME, |_, event| {
            Box::pin(async move {
                let error_data = event.data::<ErrorEventData>()?;
                log::warn!(
                    "Received Error Response from Server: {} - {}",
                    error_data.code,
                    error_data.message
                );

                Ok(())
            })
        });
        Self {
            handler,
            address: None,
        }
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

    /// Builds an ipc server
    pub async fn build_server(self) -> Result<()> {
        self.validate()?;
        let server = IPCServer {
            handler: self.handler,
        };
        server.start(&self.address.unwrap()).await?;

        Ok(())
    }

    /// Builds an ipc client
    pub async fn build_client(self) -> Result<StreamEmitter> {
        self.validate()?;
        let client = IPCClient {
            handler: self.handler,
        };

        let emitter = client.connect(&self.address.unwrap()).await?;

        Ok(emitter)
    }

    /// Validates that all required fields have been provided
    fn validate(&self) -> Result<()> {
        if self.address.is_none() {
            Err(Error::BuildError("Missing Address".to_string()))
        } else {
            Ok(())
        }
    }
}
