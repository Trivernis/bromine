use crate::error::Error;
use crate::IPCBuilder;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use tokio::sync::oneshot;

#[derive(Clone, Serialize, Deserialize)]
pub struct PingEventData {
    pub time: SystemTime,
    pub ttl: u8,
}

/// Starts a test IPC server
pub fn start_test_server(address: &'static str) -> oneshot::Receiver<bool> {
    let (tx, rx) = oneshot::channel();
    tokio::task::spawn(async move {
        tx.send(true).unwrap();
        IPCBuilder::new()
            .address(address)
            .on("ping", |ctx, event| {
                Box::pin(async move {
                    ctx.emitter.emit_response(event.id(), "pong", ()).await?;
                    Ok(())
                })
            })
            .on("trigger_error", |_, _| {
                Box::pin(async move { Err(Error::from("An error occurred.")) })
            })
            .build_server()
            .await
            .unwrap();
    });

    rx
}
