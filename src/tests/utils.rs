use crate::error::Error;
use crate::IPCBuilder;
use serde::{Deserialize, Serialize};
use std::net::ToSocketAddrs;
use std::time::SystemTime;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PingEventData {
    pub time: SystemTime,
    pub ttl: u8,
}

/// Starts a test IPC server
pub fn start_test_server(address: &'static str) -> oneshot::Receiver<bool> {
    let (tx, rx) = oneshot::channel();
    tokio::task::spawn(async move {
        tx.send(true).unwrap();
        IPCBuilder::<TcpListener>::new()
            .address(address.to_socket_addrs().unwrap().next().unwrap())
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
