use super::utils::PingEventData;
use crate::prelude::*;
use crate::protocol::AsyncProtocolStream;
use crate::tests::utils::start_test_server;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::net::TcpListener;
use typemap_rev::TypeMapKey;

async fn handle_ping_event<P: AsyncProtocolStream>(ctx: &Context<P>, e: Event) -> IPCResult<()> {
    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut ping_data = e.data::<PingEventData>()?;
    ping_data.time = SystemTime::now();
    ping_data.ttl -= 1;

    if ping_data.ttl > 0 {
        ctx.emitter.emit_response(e.id(), "pong", ping_data).await?;
    }

    Ok(())
}

fn get_builder_with_ping<L: AsyncStreamProtocolListener>(address: L::AddressType) -> IPCBuilder<L> {
    IPCBuilder::new()
        .on("ping", |ctx, e| Box::pin(handle_ping_event(ctx, e)))
        .timeout(Duration::from_secs(10))
        .address(address)
}

#[tokio::test]
async fn it_receives_tcp_events() {
    let socket_address = "127.0.0.1:8281".to_socket_addrs().unwrap().next().unwrap();
    it_receives_events::<TcpListener>(socket_address).await;
}

#[cfg(unix)]
#[tokio::test]
async fn it_receives_unix_socket_events() {
    let socket_path = PathBuf::from("/tmp/test_socket");
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).unwrap();
    }
    it_receives_events::<tokio::net::UnixListener>(socket_path).await;
}

async fn it_receives_events<L: 'static + AsyncStreamProtocolListener>(address: L::AddressType) {
    let builder = get_builder_with_ping::<L>(address.clone());
    let server_running = Arc::new(AtomicBool::new(false));

    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = get_builder_with_ping::<L>(address);
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });
    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let pool = builder.build_pooled_client(8).await.unwrap();
    let reply = pool
        .acquire()
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
        .await_reply(&pool.acquire())
        .await
        .unwrap();
    assert_eq!(reply.name(), "pong");
}

fn get_builder_with_ping_namespace(address: &str) -> IPCBuilder<TcpListener> {
    IPCBuilder::new()
        .namespace("mainspace")
        .on("ping", callback!(handle_ping_event))
        .build()
        .address(address.to_socket_addrs().unwrap().next().unwrap())
}

pub struct TestNamespace;

impl TestNamespace {
    async fn ping<P: AsyncProtocolStream>(_c: &Context<P>, _e: Event) -> IPCResult<()> {
        println!("Ping received");
        Ok(())
    }
}

impl NamespaceProvider for TestNamespace {
    fn name() -> &'static str {
        "Test"
    }

    fn register<S: AsyncProtocolStream>(handler: &mut EventHandler<S>) {
        events!(handler,
            "ping" => Self::ping,
            "ping2" => Self::ping
        );
    }
}

#[tokio::test]
async fn it_receives_namespaced_events() {
    let builder = get_builder_with_ping_namespace("127.0.0.1:8282");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = get_builder_with_ping_namespace("127.0.0.1:8282");
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

fn get_builder_with_error_handling(
    error_occurred: Arc<AtomicBool>,
    address: &str,
) -> IPCBuilder<TcpListener> {
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
        .address(address.to_socket_addrs().unwrap().next().unwrap())
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
    let ctx = IPCBuilder::<TcpListener>::new()
        .address(ADDRESS.to_socket_addrs().unwrap().next().unwrap())
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
        .await;
    assert!(reply.is_err());
}
