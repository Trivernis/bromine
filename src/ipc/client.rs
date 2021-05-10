use crate::error::Result;
use crate::events::event::Event;
use crate::events::event_handler::EventHandler;
use crate::ipc::context::Context;
use crate::ipc::stream_emitter::StreamEmitter;
use tokio::net::TcpStream;

pub struct IPCClient {
    pub(crate) handler: EventHandler,
}

impl IPCClient {
    /// Connects to a given address and returns an emitter for events to that address
    pub async fn connect(self, address: &str) -> Result<StreamEmitter> {
        let stream = TcpStream::connect(address).await?;
        let (mut read_half, write_half) = stream.into_split();
        let emitter = StreamEmitter::new(write_half);
        let ctx = Context::new(StreamEmitter::clone(&emitter));
        let handler = self.handler;

        tokio::spawn(async move {
            while let Ok(event) = Event::from_async_read(&mut read_half).await {
                if let Err(e) = handler.handle_event(&ctx, event).await {
                    log::error!("Failed to handle event: {:?}", e);
                }
            }
        });

        Ok(emitter)
    }
}
