use crate::error::{Error, Result};
use crate::events::generate_event_id;
use crate::events::payload::EventReceivePayload;
#[cfg(feature = "serialize")]
use crate::payload::SerdePayload;
use crate::prelude::{IPCError, IPCResult};
use byteorder::{BigEndian, ReadBytesExt};
#[cfg(feature = "serialize")]
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::io::{Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt};

pub const FORMAT_VERSION: [u8; 3] = [0, 9, 0];

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

    /// Decodes the payload to the given type implementing the receive payload trait
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn payload<T: EventReceivePayload>(&self) -> Result<T> {
        let payload = T::from_payload_bytes(&self.data[..])?;

        Ok(payload)
    }

    #[cfg(feature = "serialize")]
    /// Decodes the payload to the given type implementing DeserializeOwned
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn serde_payload<T: DeserializeOwned>(&self) -> Result<T> {
        let payload = SerdePayload::<T>::from_payload_bytes(&self.data[..])?;

        Ok(payload.data())
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

        let mut header_bytes = vec![0u8; header_length as usize];
        reader.read_exact(&mut header_bytes).await?;
        // additional header fields can be added a the end because when reading they will just be ignored
        let header: EventHeader = EventHeader::from_read(&mut Cursor::new(header_bytes))?;

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
        let mut buf = FORMAT_VERSION.to_vec();
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
    pub fn from_read<R: Read>(reader: &mut R) -> Result<Self> {
        Self::read_version(reader)?;
        let id = reader.read_u64::<BigEndian>()?;
        let ref_id = Self::read_ref_id(reader)?;
        let namespace_len = reader.read_u16::<BigEndian>()?;
        let namespace = Self::read_namespace(reader, namespace_len)?;
        let name = Self::read_name(reader)?;

        Ok(Self {
            id,
            ref_id,
            namespace,
            name,
        })
    }

    /// Reads and validates the format version
    fn read_version<R: Read>(reader: &mut R) -> IPCResult<Vec<u8>> {
        let mut version = vec![0u8; 3];
        reader.read_exact(&mut version)?;

        if version[0] != FORMAT_VERSION[0] {
            return Err(IPCError::unsupported_version_vec(version));
        }

        Ok(version)
    }

    /// Reads the reference event id
    fn read_ref_id<R: Read>(reader: &mut R) -> IPCResult<Option<u64>> {
        let ref_id_exists = reader.read_u8()?;
        let ref_id = match ref_id_exists {
            0x00 => None,
            0xFF => Some(reader.read_u64::<BigEndian>()?),
            _ => return Err(Error::CorruptedEvent),
        };

        Ok(ref_id)
    }

    /// Reads the name of the event
    fn read_name<R: Read>(reader: &mut R) -> IPCResult<String> {
        let name_len = reader.read_u16::<BigEndian>()?;

        Self::read_string(reader, name_len as usize)
    }

    /// Reads the namespace of the event
    fn read_namespace<R: Read>(reader: &mut R, namespace_len: u16) -> IPCResult<Option<String>> {
        let namespace = if namespace_len > 0 {
            Some(Self::read_string(reader, namespace_len as usize)?)
        } else {
            None
        };

        Ok(namespace)
    }

    fn read_string<R: Read>(reader: &mut R, length: usize) -> IPCResult<String> {
        let mut string_buf = vec![0u8; length];
        reader.read_exact(&mut string_buf)?;
        String::from_utf8(string_buf).map_err(|_| Error::CorruptedEvent)
    }
}
