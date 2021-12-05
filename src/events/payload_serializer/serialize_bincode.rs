use crate::payload::SerializationResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

pub fn serialize<T: Serialize>(data: T) -> SerializationResult<Vec<u8>> {
    let bytes = bincode::serialize(&data)?;

    Ok(bytes)
}

pub fn deserialize<R: Read, T: DeserializeOwned>(reader: R) -> SerializationResult<T> {
    let type_data = bincode::deserialize_from(reader)?;
    Ok(type_data)
}
