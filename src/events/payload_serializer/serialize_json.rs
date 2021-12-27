use crate::payload::SerializationResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

#[inline]
pub fn serialize<T: Serialize>(data: T) -> SerializationResult<Vec<u8>> {
    let bytes = serde_json::to_vec(&data)?;

    Ok(bytes)
}

#[inline]
pub fn deserialize<R: Read, T: DeserializeOwned>(reader: R) -> SerializationResult<T> {
    let type_data = serde_json::from_reader(reader)?;

    Ok(type_data)
}
