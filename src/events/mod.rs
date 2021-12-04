use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod error_event;
pub mod event;
pub mod event_handler;
pub mod payload;

#[cfg(feature = "serialize")]
pub mod payload_serializer;


/// Generates a new event id
pub(crate) fn generate_event_id() -> u64 {
    lazy_static::lazy_static! {
        static ref COUNTER: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    }
    let epoch_elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    (epoch_elapsed.as_secs() % u16::MAX as u64) << 48
        | (epoch_elapsed.subsec_millis() as u64) << 32
        | COUNTER.fetch_add(1, Ordering::SeqCst)
}
