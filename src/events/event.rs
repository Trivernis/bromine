use crate::error::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

/// A container representing an event and underlying binary data.
/// The data can be decoded into an object representation or read
/// as raw binary data.
#[derive(Serialize, Deserialize)]
pub struct Event {
    name: String,
    data: Vec<u8>,
}

impl Event {
    /// Creates a new event
    pub fn new(name: String, data: Vec<u8>) -> Self {
        Self { name, data }
    }

    /// Decodes the data to the given type
    pub fn data<T: DeserializeOwned>(&self) -> Result<T> {
        let data = rmp_serde::from_read(&self.data[..])?;

        Ok(data)
    }

    /// Returns a reference of the underlying data
    pub fn data_raw(&self) -> &[u8] {
        &self.data
    }

    /// Returns the name of the event
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reads an event message
    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u32().await?;
        let mut data = vec![0u8; length as usize];
        reader.read_exact(&mut data).await?;
        let event = rmp_serde::from_read(&data[..])?;

        Ok(event)
    }

    /// Encodes the event into bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut event_bytes = rmp_serde::to_vec(&self)?;
        let mut length_bytes = (event_bytes.len() as u32).to_be_bytes().to_vec();
        length_bytes.reverse();

        for byte in length_bytes {
            event_bytes.insert(0, byte);
        }

        Ok(event_bytes)
    }
}
