use crate::events::event_handler::EventHandler;
use crate::namespace::Namespace;

pub trait NamespaceProvider {
    fn name() -> String;
    fn register(handler: &mut EventHandler);
}

impl Namespace {
    pub fn from_provider<N: NamespaceProvider>() -> Self {
        let name = N::name();
        let mut handler = EventHandler::new();
        N::register(&mut handler);

        Self::new(name, handler)
    }
}
