use crate::events::event_handler::EventHandler;
use std::sync::Arc;

#[derive(Clone)]
pub struct Namespace {
    name: String,
    pub(crate) handler: Arc<EventHandler>,
}

impl Namespace {
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
