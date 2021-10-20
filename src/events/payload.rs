use crate::prelude::IPCResult;
use serde::Serialize;

pub trait EventSendPayload {
    fn to_payload_bytes(self) -> IPCResult<Vec<u8>>;
}

impl<T> EventSendPayload for T
where
    T: Serialize,
{
    fn to_payload_bytes(self) -> IPCResult<Vec<u8>> {
        let bytes = rmp_serde::to_vec(&self)?;

        Ok(bytes)
    }
}
