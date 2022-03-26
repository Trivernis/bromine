use crate::payload::SerializationResult;
use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

#[inline]
pub fn serialize<T: Serialize>(data: T) -> SerializationResult<Bytes> {
    let bytes = rmp_serde::to_vec(&data)?;

    Ok(Bytes::from(bytes))
}

#[inline]
pub fn deserialize<R: Read, T: DeserializeOwned>(reader: R) -> SerializationResult<T> {
    let type_data = rmp_serde::from_read(reader)?;
    Ok(type_data)
}
