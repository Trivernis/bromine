use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

pub fn get_free_port() -> u8 {
    lazy_static! {
        static ref PORT_COUNTER: Arc<AtomicU8> = Arc::new(AtomicU8::new(0));
    }
    PORT_COUNTER.fetch_add(1, Ordering::Relaxed)
}
