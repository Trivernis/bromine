use crate::events::event_handler::EventHandler;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Namespace {
    name: String,
    pub(crate) handler: Arc<EventHandler>,
}

impl Namespace {
    /// Creates a new namespace with an event handler to register event callbacks on
    pub fn new<S: ToString>(name: S, handler: EventHandler) -> Self {
        Self {
            name: name.to_string(),
            handler: Arc::new(handler),
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}
