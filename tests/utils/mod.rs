use bromine::context::Context;
use bromine::protocol::AsyncStreamProtocolListener;
use bromine::IPCBuilder;
use call_counter::*;
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::oneshot::channel;

pub mod call_counter;
pub mod protocol;

pub fn get_free_port() -> u8 {
    lazy_static! {
        static ref PORT_COUNTER: Arc<AtomicU8> = Arc::new(AtomicU8::new(0));
    }
    PORT_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub async fn start_server_and_client<
    F: Fn() -> IPCBuilder<L> + Send + Sync + 'static,
    L: AsyncStreamProtocolListener,
>(
    builder_fn: F,
) -> Context {
    let counters = CallCounter::default();
    let (sender, receiver) = channel::<()>();
    let client_builder = builder_fn().insert::<CallCounterKey>(counters.clone());

    tokio::task::spawn({
        async move {
            sender.send(()).unwrap();
            builder_fn()
                .insert::<CallCounterKey>(counters)
                .build_server()
                .await
                .unwrap()
        }
    });
    receiver.await.unwrap();

    let ctx = client_builder.build_client().await.unwrap();

    ctx
}
