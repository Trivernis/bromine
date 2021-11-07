use crate::events::event_handler::EventHandler;
use crate::protocol::AsyncProtocolStream;
use std::sync::Arc;

#[derive(Debug)]
pub struct Namespace<S: AsyncProtocolStream> {
    name: String,
    pub(crate) handler: Arc<EventHandler<S>>,
}

impl<S> Clone for Namespace<S>
where
    S: AsyncProtocolStream,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl<S> Namespace<S>
where
    S: AsyncProtocolStream,
{
    /// Creates a new namespace with an event handler to register event callbacks on
    pub fn new<S2: ToString>(name: S2, handler: EventHandler<S>) -> Self {
        Self {
            name: name.to_string(),
            handler: Arc::new(handler),
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}
