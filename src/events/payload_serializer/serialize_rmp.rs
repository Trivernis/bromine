use crate::payload::{EventReceivePayload, EventSendPayload};
use crate::prelude::{IPCError, IPCResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SerializationError {
    #[error("failed to serialize with rmp: {0}")]
    Serialize(#[from] rmp_serde::encode::Error),

    #[error("failed to deserialize with rmp: {0}")]
    Deserialize(#[from] rmp_serde::decode::Error),
}

impl From<rmp_serde::decode::Error> for IPCError {
    fn from(e: rmp_serde::decode::Error) -> Self {
        IPCError::Serialization(SerializationError::Deserialize(e))
    }
}

impl From<rmp_serde::encode::Error> for IPCError {
    fn from(e: rmp_serde::encode::Error) -> Self {
        IPCError::Serialization(SerializationError::Serialize(e))
    }
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

impl<T> EventReceivePayload for T
where
    T: DeserializeOwned,
{
    fn from_payload_bytes<R: Read>(reader: R) -> IPCResult<Self> {
        let type_data = rmp_serde::from_read(reader)?;
        Ok(type_data)
    }
}
