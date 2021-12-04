use crate::payload::{EventReceivePayload, EventSendPayload};
use crate::prelude::IPCResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

pub type SerializationError = serde_json::Error;

impl<T> EventSendPayload for T
where
    T: Serialize,
{
    fn to_payload_bytes(self) -> IPCResult<Vec<u8>> {
        let bytes = serde_json::to_vec(&self)?;

        Ok(bytes)
    }
}

impl<T> EventReceivePayload for T
where
    T: DeserializeOwned,
{
    fn from_payload_bytes<R: Read>(reader: R) -> IPCResult<Self> {
        let type_data = serde_json::from_reader(reader)?;

        Ok(type_data)
    }
}
