use crate::payload::{EventReceivePayload, EventSendPayload};
use crate::prelude::IPCResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

pub type SerializationError = postcard::Error;

impl<T> EventSendPayload for T
where
    T: Serialize,
{
    fn to_payload_bytes(self) -> IPCResult<Vec<u8>> {
        let bytes = postcard::to_allocvec(&self)?.to_vec();

        Ok(bytes)
    }
}

impl<T> EventReceivePayload for T
where
    T: DeserializeOwned,
{
    fn from_payload_bytes<R: Read>(mut reader: R) -> IPCResult<Self> {
        let mut buf = Vec::new();
        // reading to end means reading the full size of the provided data
        reader.read_to_end(&mut buf)?;
        let type_data = postcard::from_bytes(&buf)?;

        Ok(type_data)
    }
}
