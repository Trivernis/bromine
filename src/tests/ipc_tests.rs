use self::super::utils::PingEventData;
use crate::prelude::*;
use crate::tests::utils::start_test_server;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use typemap_rev::TypeMapKey;

async fn handle_ping_event(ctx: &Context, e: Event) -> IPCResult<()> {
    let mut ping_data = e.data::<PingEventData>()?;
    ping_data.time = SystemTime::now();
    ping_data.ttl -= 1;

    if ping_data.ttl > 0 {
        ctx.emitter.emit_response(e.id(), "pong", ping_data).await?;
    }

    Ok(())
}

fn get_builder_with_ping(address: &str) -> IPCBuilder {
    IPCBuilder::new()
        .on("ping", |ctx, e| Box::pin(handle_ping_event(ctx, e)))
        .address(address)
}

#[tokio::test]
async fn it_receives_events() {
    let builder = get_builder_with_ping("127.0.0.1:8281");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = get_builder_with_ping("127.0.0.1:8281");
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

fn get_builder_with_ping_mainspace(address: &str) -> IPCBuilder {
    IPCBuilder::new()
        .namespace("mainspace")
        .on("ping", callback!(handle_ping_event))
        .build()
        .address(address)
}

pub struct TestNamespace;

impl TestNamespace {
    async fn ping(_c: &Context, _e: Event) -> IPCResult<()> {
        println!("Ping received");
        Ok(())
    }
}

impl NamespaceProvider for TestNamespace {
    fn name() -> &'static str {
        "Test"
    }

    fn register(handler: &mut EventHandler) {
        events!(handler,
            "ping" => Self::ping,
            "ping2" => Self::ping
        );
    }
}

#[tokio::test]
async fn it_receives_namespaced_events() {
    let builder = get_builder_with_ping_mainspace("127.0.0.1:8282");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = get_builder_with_ping_mainspace("127.0.0.1:8282");
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });
    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let ctx = builder
        .add_namespace(namespace!(TestNamespace))
        .build_client()
        .await
        .unwrap();
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

struct ErrorOccurredKey;

impl TypeMapKey for ErrorOccurredKey {
    type Value = Arc<AtomicBool>;
}

fn get_builder_with_error_handling(error_occurred: Arc<AtomicBool>, address: &str) -> IPCBuilder {
    IPCBuilder::new()
        .insert::<ErrorOccurredKey>(error_occurred)
        .on("ping", move |_, _| {
            Box::pin(async move { Err(IPCError::from("ERRROROROROR")) })
        })
        .on(
            "error",
            callback!(ctx, event, async move {
                let error = event.data::<error_event::ErrorEventData>()?;
                assert!(error.message.len() > 0);
                assert_eq!(error.code, 500);
                {
                    let data = ctx.data.read().await;
                    let error_occurred = data.get::<ErrorOccurredKey>().unwrap();
                    error_occurred.store(true, Ordering::SeqCst);
                }
                Ok(())
            }),
        )
        .address(address)
}

#[tokio::test]
async fn it_handles_errors() {
    let error_occurred = Arc::new(AtomicBool::new(false));
    let builder = get_builder_with_error_handling(Arc::clone(&error_occurred), "127.0.0.1:8283");
    let server_running = Arc::new(AtomicBool::new(false));

    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let error_occurred = Arc::clone(&error_occurred);
        let builder = get_builder_with_error_handling(error_occurred, "127.0.0.1:8283");
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
