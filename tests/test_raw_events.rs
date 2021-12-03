mod test_protocol;

use bromine::prelude::*;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use test_protocol::*;
use tokio::sync::oneshot::channel;
use tokio::sync::RwLock;
use typemap_rev::TypeMapKey;

#[tokio::test]
async fn it_sends_events() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let counters = {
        let data = ctx.data.read().await;
        data.get::<CallCounterKey>().unwrap().clone()
    };
    ctx.emitter.emit("ping", ()).await.unwrap();

    // allow the event to be processed
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(counters.get("ping").await, 1);
    assert_eq!(counters.get("pong").await, 1);
}

struct CallCounterKey;

impl TypeMapKey for CallCounterKey {
    type Value = CallCounter;
}

#[derive(Clone, Default, Debug)]
struct CallCounter {
    inner: Arc<RwLock<HashMap<String, AtomicUsize>>>,
}

impl CallCounter {
    pub async fn incr(&self, name: &str) {
        {
            let calls = self.inner.read().await;
            if let Some(call) = calls.get(name) {
                call.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
        {
            let mut calls = self.inner.write().await;
            calls.insert(name.to_string(), AtomicUsize::new(1));
        }
    }

    pub async fn get(&self, name: &str) -> usize {
        let calls = self.inner.read().await;

        calls
            .get(name)
            .map(|n| n.load(Ordering::SeqCst))
            .unwrap_or(0)
    }
}

fn get_free_port() -> u8 {
    lazy_static! {
        static ref PORT_COUNTER: Arc<AtomicU8> = Arc::new(AtomicU8::new(0));
    }
    PORT_COUNTER.fetch_add(1, Ordering::Relaxed)
}

async fn get_client_with_server(port: u8) -> Context {
    let counters = CallCounter::default();
    let (sender, receiver) = channel::<()>();

    tokio::task::spawn({
        let counters = counters.clone();

        async move {
            sender.send(()).unwrap();
            get_builder(port)
                .insert::<CallCounterKey>(counters)
                .build_server()
                .await
                .unwrap()
        }
    });
    receiver.await.unwrap();

    get_builder(port)
        .insert::<CallCounterKey>(counters)
        .build_client()
        .await
        .unwrap()
}

fn get_builder(port: u8) -> IPCBuilder<TestProtocolListener> {
    IPCBuilder::new()
        .address(port)
        .on(
            "ping",
            callback!(
                ctx,
                event,
                async move { handle_ping_event(ctx, event).await }
            ),
        )
        .timeout(Duration::from_millis(100))
        .on(
            "pong",
            callback!(
                ctx,
                event,
                async move { handle_pong_event(ctx, event).await }
            ),
        )
}

async fn increment_counter_for_event(ctx: &Context, event: &Event) {
    let data = ctx.data.read().await;
    data.get::<CallCounterKey>()
        .unwrap()
        .incr(event.name())
        .await;
}

async fn handle_ping_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;
    ctx.emitter.emit_response(event.id(), "pong", ()).await?;

    Ok(())
}

async fn handle_pong_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;
    Ok(())
}
