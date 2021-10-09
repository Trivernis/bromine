use crate::context::Context;
use crate::error::Result;
use crate::events::event_handler::EventHandler;
use crate::namespaces::namespace::Namespace;
use crate::{Event, IPCBuilder};
use std::future::Future;
use std::pin::Pin;

pub struct NamespaceBuilder {
    name: String,
    handler: EventHandler,
    ipc_builder: IPCBuilder,
}

impl NamespaceBuilder {
    pub(crate) fn new(ipc_builder: IPCBuilder, name: String) -> Self {
        Self {
            name,
            handler: EventHandler::new(),
            ipc_builder,
        }
    }

    /// Adds an event callback on the namespace
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

    /// Builds the namespace
    pub fn build(self) -> IPCBuilder {
        let namespace = Namespace::new(self.name, self.handler);
        self.ipc_builder.add_namespace(namespace)
    }
}
