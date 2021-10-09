use self::super::utils::PingEventData;
use crate::context::Context;
use crate::error::Error;
use crate::error::Result;
use crate::events::error_event::ErrorEventData;
use crate::tests::utils::start_test_server;
use crate::{Event, IPCBuilder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

async fn handle_ping_event(ctx: &Context, e: Event) -> Result<()> {
    let mut ping_data = e.data::<PingEventData>()?;
    ping_data.time = SystemTime::now();
    ping_data.ttl -= 1;

    if ping_data.ttl > 0 {
        ctx.emitter.emit_response(e.id(), "pong", ping_data).await?;
    }

    Ok(())
}

#[tokio::test]
async fn it_receives_events() {
    let builder = IPCBuilder::new()
        .on("ping", |ctx, e| Box::pin(handle_ping_event(ctx, e)))
        .address("127.0.0.1:8281");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = builder.clone();
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });
    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let ctx = builder.build_client().await.unwrap();
    let reply = ctx
        .emitter
        .emit(
            "ping",
            PingEventData {
                ttl: 16,
                time: SystemTime::now(),
            },
        )
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    assert_eq!(reply.name(), "pong");
}

#[tokio::test]
async fn it_receives_namespaced_events() {
    let builder = IPCBuilder::new()
        .namespace("mainspace")
        .on("ping", |ctx, e| Box::pin(handle_ping_event(ctx, e)))
        .build()
        .address("127.0.0.1:8282");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = builder.clone();
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });
    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let ctx = builder.build_client().await.unwrap();
    let reply = ctx
        .emitter
        .emit_to(
            "mainspace",
            "ping",
            PingEventData {
                ttl: 16,
                time: SystemTime::now(),
            },
        )
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    assert_eq!(reply.name(), "pong");
}

#[tokio::test]
async fn it_handles_errors() {
    let error_occurred = Arc::new(AtomicBool::new(false));
    let builder = IPCBuilder::new()
        .on("ping", move |_, _| {
            Box::pin(async move { Err(Error::from("ERRROROROROR")) })
        })
        .on("error", {
            let error_occurred = Arc::clone(&error_occurred);
            move |_, e| {
                let error_occurred = Arc::clone(&error_occurred);
                Box::pin(async move {
                    let error = e.data::<ErrorEventData>()?;
                    assert!(error.message.len() > 0);
                    assert_eq!(error.code, 500);
                    error_occurred.store(true, Ordering::SeqCst);
                    Ok(())
                })
            }
        })
        .address("127.0.0.1:8283");
    let server_running = Arc::new(AtomicBool::new(false));

    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = builder.clone();
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });

    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let ctx = builder.build_client().await.unwrap();
    ctx.emitter.emit("ping", ()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(error_occurred.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_error_responses() {
    static ADDRESS: &str = "127.0.0.1:8284";
    start_test_server(ADDRESS).await.unwrap();
    let ctx = IPCBuilder::new()
        .address(ADDRESS)
        .build_client()
        .await
        .unwrap();
    let reply = ctx
        .emitter
        .emit("ping", ())
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    assert_eq!(reply.name(), "pong");

    let reply = ctx
        .emitter
        .emit("trigger_error", ())
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    assert_eq!(reply.name(), "error");
}
