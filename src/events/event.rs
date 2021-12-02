use crate::error::{Error, Result};
use crate::events::generate_event_id;
use crate::events::payload::EventReceivePayload;
use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncReadExt};

/// A container representing an event and underlying binary data.
/// The data can be decoded into an object representation or read
/// as raw binary data.
#[derive(Debug)]
pub struct Event {
    header: EventHeader,
    data: Vec<u8>,
}

#[derive(Debug)]
struct EventHeader {
    id: u64,
    ref_id: Option<u64>,
    namespace: Option<String>,
    name: String,
}

impl Event {
    /// Creates a new event with a namespace
    #[tracing::instrument(level = "trace", skip(data))]
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
    #[tracing::instrument(level = "trace", skip(data))]
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
    #[tracing::instrument(level = "trace", skip(self))]
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
    #[tracing::instrument(level = "trace", skip(reader))]
    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let total_length = reader.read_u64().await?;
        let header_length = reader.read_u16().await?;
        let data_length = total_length - header_length as u64;
        tracing::trace!(total_length, header_length, data_length);

        let header: EventHeader = EventHeader::from_async_read(reader).await?;

        let mut data = vec![0u8; data_length as usize];
        reader.read_exact(&mut data).await?;
        let event = Event { header, data };

        Ok(event)
    }

    /// Encodes the event into bytes
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn into_bytes(mut self) -> Result<Vec<u8>> {
        let mut header_bytes = self.header.into_bytes();
        let header_length = header_bytes.len() as u16;
        let data_length = self.data.len();
        let total_length = header_length as u64 + data_length as u64;
        tracing::trace!(total_length, header_length, data_length);

        let mut buf = Vec::with_capacity(total_length as usize);
        buf.append(&mut total_length.to_be_bytes().to_vec());
        buf.append(&mut header_length.to_be_bytes().to_vec());
        buf.append(&mut header_bytes);
        buf.append(&mut self.data);

        Ok(buf)
    }
}

impl EventHeader {
    /// Serializes the event header into bytes
    pub fn into_bytes(self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.append(&mut self.id.to_be_bytes().to_vec());

        if let Some(ref_id) = self.ref_id {
            buf.push(0xFF);
            buf.append(&mut ref_id.to_be_bytes().to_vec());
        } else {
            buf.push(0x00);
        }
        if let Some(namespace) = self.namespace {
            let namespace_len = namespace.len() as u16;
            buf.append(&mut namespace_len.to_be_bytes().to_vec());
            buf.append(&mut namespace.into_bytes());
        } else {
            buf.append(&mut 0u16.to_be_bytes().to_vec());
        }
        let name_len = self.name.len() as u16;
        buf.append(&mut name_len.to_be_bytes().to_vec());
        buf.append(&mut self.name.into_bytes());

        buf
    }

    /// Parses an event header from an async reader
    pub async fn from_async_read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Self> {
        let id = reader.read_u64().await?;
        let ref_id_exists = reader.read_u8().await?;
        let ref_id = match ref_id_exists {
            0x00 => None,
            0xFF => Some(reader.read_u64().await?),
            _ => return Err(Error::CorruptedEvent),
        };
        let namespace_len = reader.read_u16().await?;

        let namespace = if namespace_len > 0 {
            let mut namespace_buf = vec![0u8; namespace_len as usize];
            reader.read_exact(&mut namespace_buf).await?;
            Some(String::from_utf8(namespace_buf).map_err(|_| Error::CorruptedEvent)?)
        } else {
            None
        };
        let name_len = reader.read_u16().await?;
        let mut name_buf = vec![0u8; name_len as usize];
        reader.read_exact(&mut name_buf).await?;
        let name = String::from_utf8(name_buf).map_err(|_| Error::CorruptedEvent)?;

        Ok(Self {
            id,
            ref_id,
            namespace,
            name,
        })
    }
}
