use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Clone, Serialize, Deserialize)]
pub struct PingEventData {
    pub time: SystemTime,
    pub ttl: u8,
}
