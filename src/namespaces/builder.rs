use crate::error::Result;
use crate::event::Event;
use crate::event_handler::Response;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::namespaces::namespace::Namespace;
use crate::protocol::AsyncStreamProtocolListener;
use crate::IPCBuilder;
use std::future::Future;
use std::pin::Pin;

pub struct NamespaceBuilder<L: AsyncStreamProtocolListener> {
    name: String,
    handler: EventHandler,
    ipc_builder: IPCBuilder<L>,
}

impl<L> NamespaceBuilder<L>
where
    L: AsyncStreamProtocolListener,
{
    pub(crate) fn new(ipc_builder: IPCBuilder<L>, name: String) -> Self {
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
            ) -> Pin<Box<(dyn Future<Output = Result<Response>> + Send + 'a)>>
            + Send
            + Sync,
    {
        self.handler.on(event, callback);

        self
    }

    /// Builds the namespace
    #[tracing::instrument(skip(self))]
    pub fn build(self) -> IPCBuilder<L> {
        let namespace = Namespace::new(self.name, self.handler);
        self.ipc_builder.add_namespace(namespace)
    }
}
