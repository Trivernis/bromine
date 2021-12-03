use bromine::context::Context;
use bromine::event::Event;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use typemap_rev::TypeMapKey;

pub async fn get_counter_from_context(ctx: &Context) -> CallCounter {
    let data = ctx.data.read().await;

    data.get::<CallCounterKey>().unwrap().clone()
}

pub async fn increment_counter_for_event(ctx: &Context, event: &Event) {
    let data = ctx.data.read().await;
    data.get::<CallCounterKey>()
        .unwrap()
        .incr(event.name())
        .await;
}

pub struct CallCounterKey;

impl TypeMapKey for CallCounterKey {
    type Value = CallCounter;
}

#[derive(Clone, Default, Debug)]
pub struct CallCounter {
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
