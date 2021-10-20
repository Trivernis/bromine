use crate::error::Result;
use crate::events::generate_event_id;
use crate::events::payload::EventReceivePayload;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

/// A container representing an event and underlying binary data.
/// The data can be decoded into an object representation or read
/// as raw binary data.
#[derive(Serialize, Deserialize)]
pub struct Event {
    id: u64,
    ref_id: Option<u64>,
    namespace: Option<String>,
    name: String,
    data: Vec<u8>,
}

impl Event {
    /// Creates a new event with a namespace
    pub fn with_namespace(
        namespace: String,
        name: String,
        data: Vec<u8>,
        ref_id: Option<u64>,
    ) -> Self {
        Self {
            id: generate_event_id(),
            ref_id,
            namespace: Some(namespace),
            name,
            data,
        }
    }

    /// Creates a new event
    pub fn new(name: String, data: Vec<u8>, ref_id: Option<u64>) -> Self {
        Self {
            id: generate_event_id(),
            ref_id,
            namespace: None,
            name,
            data,
        }
    }

    /// Decodes the data to the given type
    pub fn data<T: EventReceivePayload>(&self) -> Result<T> {
        let data = T::from_payload_bytes(&self.data[..])?;

        Ok(data)
    }

    /// Returns a reference of the underlying data
    pub fn data_raw(&self) -> &[u8] {
        &self.data
    }

    /// Returns a reference to the namespace
    pub fn namespace(&self) -> &Option<String> {
        &self.namespace
    }

    /// Returns the name of the event
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reads an event message
    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let length = reader.read_u32().await?;
        log::trace!("Parsing event of length {}", length);
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

    /// The identifier of the message
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The ID of the message referenced by this message.
    /// It represents the message that is replied to and can be None.
    pub fn reference_id(&self) -> Option<u64> {
        self.ref_id.clone()
    }
}
