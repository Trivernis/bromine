use crate::events::event_handler::EventHandler;
use crate::namespace::Namespace;
use crate::protocol::AsyncProtocolStream;

pub trait NamespaceProvider {
    fn name() -> &'static str;
    fn register<S: AsyncProtocolStream>(handler: &mut EventHandler<S>);
}

impl<S> Namespace<S>
where
    S: AsyncProtocolStream,
{
    pub fn from_provider<N: NamespaceProvider>() -> Self {
        let name = N::name();
        let mut handler = EventHandler::new();
        N::register(&mut handler);

        Self::new(name, handler)
    }
}
