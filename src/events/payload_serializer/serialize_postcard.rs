use crate::payload::SerializationResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;

#[inline]
pub fn serialize<T: Serialize>(data: T) -> SerializationResult<Vec<u8>> {
    let bytes = postcard::to_allocvec(&data)?.to_vec();

    Ok(bytes)
}

#[inline]
pub fn deserialize<R: Read, T: DeserializeOwned>(mut reader: R) -> SerializationResult<T> {
    let mut buf = Vec::new();
    // reading to end means reading the full size of the provided data
    reader.read_to_end(&mut buf)?;
    let type_data = postcard::from_bytes(&buf)?;

    Ok(type_data)
}
