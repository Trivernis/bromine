use crate::prelude::IPCResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

/// Trait to convert event data into sending bytes
/// It is implemented for all types that implement Serialize
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

/// Trait to get the event data from receiving bytes.
/// It is implemented for all types that are DeserializeOwned
pub trait EventReceivePayload: Sized {
    fn from_payload_bytes<R: Read>(reader: R) -> IPCResult<Self>;
}

impl<T> EventReceivePayload for T
where
    T: DeserializeOwned,
{
    fn from_payload_bytes<R: Read>(reader: R) -> IPCResult<Self> {
        let type_data = rmp_serde::from_read(reader)?;
        Ok(type_data)
    }
}

/// A payload wrapper type for sending bytes directly without
/// serializing them
#[derive(Clone, Debug)]
pub struct BytePayload {
    bytes: Vec<u8>,
}

impl BytePayload {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl EventSendPayload for BytePayload {
    fn to_payload_bytes(self) -> IPCResult<Vec<u8>> {
        Ok(self.bytes)
    }
}

impl EventReceivePayload for BytePayload {
    fn from_payload_bytes<R: Read>(mut reader: R) -> IPCResult<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        Ok(Self::new(buf))
    }
}
