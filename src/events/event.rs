use crate::error::{Error, Result};
use crate::events::generate_event_id;
use crate::events::payload::FromPayload;
use byteorder::{BigEndian, ReadBytesExt};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::convert::TryFrom;
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
    event_type: EventType,
    ref_id: Option<u64>,
    namespace: Option<String>,
    name: String,
}

#[derive(Clone, Debug, TryFromPrimitive, IntoPrimitive, Copy, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum EventType {
    Initiator,
    Response,
    End,
    Error,
}

impl Event {
    /// Creates a new event that acts as an initiator for further response events
    #[tracing::instrument(level = "trace", skip(data))]
    #[inline]
    pub fn initiator(namespace: Option<String>, name: String, data: Vec<u8>) -> Self {
        Self::new(namespace, name, data, None, EventType::Initiator)
    }

    /// Creates a new event that is a response to a previous event
    #[tracing::instrument(level = "trace", skip(data))]
    #[inline]
    pub fn response(namespace: Option<String>, name: String, data: Vec<u8>, ref_id: u64) -> Self {
        Self::new(namespace, name, data, Some(ref_id), EventType::Response)
    }

    /// Creates a new error event as a response to a previous event
    #[tracing::instrument(level = "trace", skip(data))]
    #[inline]
    pub fn error(namespace: Option<String>, name: String, data: Vec<u8>, ref_id: u64) -> Self {
        Self::new(namespace, name, data, Some(ref_id), EventType::Error)
    }

    /// Creates a new event that indicates the end of a series of responses (in an event handler)
    /// and might contain a final response payload
    #[tracing::instrument(level = "trace", skip(data))]
    #[inline]
    pub fn end(namespace: Option<String>, name: String, data: Vec<u8>, ref_id: u64) -> Self {
        Self::new(namespace, name, data, Some(ref_id), EventType::Response)
    }

    /// Creates a new event
    #[tracing::instrument(level = "trace", skip(data))]
    pub(crate) fn new(
        namespace: Option<String>,
        name: String,
        data: Vec<u8>,
        ref_id: Option<u64>,
        event_type: EventType,
    ) -> Self {
        let header = EventHeader {
            id: generate_event_id(),
            event_type,
            ref_id,
            namespace,
            name,
        };
        Self { header, data }
    }

    /// The identifier of the message
    #[inline]
    pub fn id(&self) -> u64 {
        self.header.id
    }

    /// The type of the event
    #[inline]
    pub fn event_type(&self) -> EventType {
        self.header.event_type
    }

    /// The ID of the message referenced by this message.
    /// It represents the message that is replied to and can be None.
    #[inline]
    pub fn reference_id(&self) -> Option<u64> {
        self.header.ref_id.clone()
    }

    /// Decodes the payload to the given type implementing the receive payload trait
    #[inline]
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn payload<T: FromPayload>(&self) -> Result<T> {
        let payload = T::from_payload(&self.data[..])?;

        Ok(payload)
    }

    /// Returns a reference of the underlying data
    #[inline]
    pub fn data_raw(&self) -> &[u8] {
        &self.data
    }

    /// Returns a reference to the namespace
    #[inline]
    pub fn namespace(&self) -> &Option<String> {
        &self.header.namespace
    }

    /// Returns the name of the event
    #[inline]
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
        buf.push(self.event_type.into());

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
        let event_type_num = reader.read_u8()?;
        let event_type = EventType::try_from(event_type_num).map_err(|_| Error::CorruptedEvent)?;
        let ref_id = Self::read_ref_id(reader)?;
        let namespace_len = reader.read_u16::<BigEndian>()?;
        let namespace = Self::read_namespace(reader, namespace_len)?;
        let name = Self::read_name(reader)?;

        Ok(Self {
            id,
            event_type,
            ref_id,
            namespace,
            name,
        })
    }

    /// Reads and validates the format version
    #[inline]
    fn read_version<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
        let mut version = vec![0u8; 3];
        reader.read_exact(&mut version)?;

        if version[0] != FORMAT_VERSION[0] {
            return Err(Error::unsupported_version_vec(version));
        }

        Ok(version)
    }

    /// Reads the reference event id
    #[inline]
    fn read_ref_id<R: Read>(reader: &mut R) -> Result<Option<u64>> {
        let ref_id_exists = reader.read_u8()?;
        let ref_id = match ref_id_exists {
            0x00 => None,
            0xFF => Some(reader.read_u64::<BigEndian>()?),
            _ => return Err(Error::CorruptedEvent),
        };

        Ok(ref_id)
    }

    /// Reads the name of the event
    #[inline]
    fn read_name<R: Read>(reader: &mut R) -> Result<String> {
        let name_len = reader.read_u16::<BigEndian>()?;

        Self::read_string(reader, name_len as usize)
    }

    /// Reads the namespace of the event
    #[inline]
    fn read_namespace<R: Read>(reader: &mut R, namespace_len: u16) -> Result<Option<String>> {
        let namespace = if namespace_len > 0 {
            Some(Self::read_string(reader, namespace_len as usize)?)
        } else {
            None
        };

        Ok(namespace)
    }

    #[inline]
    fn read_string<R: Read>(reader: &mut R, length: usize) -> Result<String> {
        let mut string_buf = vec![0u8; length];
        reader.read_exact(&mut string_buf)?;
        String::from_utf8(string_buf).map_err(|_| Error::CorruptedEvent)
    }
}
