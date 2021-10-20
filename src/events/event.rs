use crate::error::Result;
use crate::events::generate_event_id;
use crate::events::payload::EventReceivePayload;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt};

/// A container representing an event and underlying binary data.
/// The data can be decoded into an object representation or read
/// as raw binary data.
pub struct Event {
    header: EventHeader,
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct EventHeader {
    id: u64,
    ref_id: Option<u64>,
    namespace: Option<String>,
    name: String,
}

impl Event {
    /// Creates a new event with a namespace
    pub fn with_namespace(
        namespace: String,
        name: String,
        data: Vec<u8>,
        ref_id: Option<u64>,
    ) -> Self {
        let header = EventHeader {
            id: generate_event_id(),
            ref_id,
            namespace: Some(namespace),
            name,
        };
        Self { header, data }
    }

    /// Creates a new event
    pub fn new(name: String, data: Vec<u8>, ref_id: Option<u64>) -> Self {
        let header = EventHeader {
            id: generate_event_id(),
            ref_id,
            namespace: None,
            name,
        };
        Self { header, data }
    }

    /// The identifier of the message
    pub fn id(&self) -> u64 {
        self.header.id
    }

    /// The ID of the message referenced by this message.
    /// It represents the message that is replied to and can be None.
    pub fn reference_id(&self) -> Option<u64> {
        self.header.ref_id.clone()
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
        &self.header.namespace
    }

    /// Returns the name of the event
    pub fn name(&self) -> &str {
        &self.header.name
    }

    /// Reads an event message
    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let total_length = reader.read_u64().await?;
        let header_length = reader.read_u16().await?;
        let data_length = total_length - header_length as u64;
        log::trace!(
            "Parsing event of length {} ({} header, {} data)",
            total_length,
            header_length,
            data_length
        );

        let header: EventHeader = {
            let mut header_bytes = vec![0u8; header_length as usize];
            reader.read_exact(&mut header_bytes).await?;
            rmp_serde::from_read(&header_bytes[..])?
        };
        let mut data = vec![0u8; data_length as usize];
        reader.read_exact(&mut data).await?;
        let event = Event { header, data };

        Ok(event)
    }

    /// Encodes the event into bytes
    pub fn into_bytes(mut self) -> Result<Vec<u8>> {
        let mut header_bytes = rmp_serde::to_vec(&self.header)?;
        let header_length = header_bytes.len() as u16;
        let data_length = self.data.len();
        let total_length = header_length as u64 + data_length as u64;

        let mut buf = Vec::with_capacity(total_length as usize);
        buf.append(&mut total_length.to_be_bytes().to_vec());
        buf.append(&mut header_length.to_be_bytes().to_vec());
        buf.append(&mut header_bytes);
        buf.append(&mut self.data);

        Ok(buf)
    }
}
