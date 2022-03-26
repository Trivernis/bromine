use crate::context::Context;
use crate::error::Result;
use crate::payload::{FromPayload, IntoPayload};
use crate::prelude::{IPCError, IPCResult};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{BufMut, Bytes, BytesMut};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;

pub static ERROR_EVENT_NAME: &str = "error";
pub static END_EVENT_NAME: &str = "end";

/// Data returned on error event.
/// The error event has a default handler that just logs that
/// an error occurred. For a custom handler, register a handler on
/// the [ERROR_EVENT_NAME] event.
#[derive(Clone, Debug)]
pub struct ErrorEventData {
    pub code: u16,
    pub message: String,
}

impl Error for ErrorEventData {}

impl Display for ErrorEventData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "IPC Code {}: '{}'", self.code, self.message)
    }
}

impl IntoPayload for ErrorEventData {
    fn into_payload(self, _: &Context) -> IPCResult<Bytes> {
        let mut buf = BytesMut::new();
        buf.put_u16(self.code);
        buf.put_u32(self.message.len() as u32);
        buf.put(Bytes::from(self.message));

        Ok(buf.freeze())
    }
}

impl FromPayload for ErrorEventData {
    fn from_payload<R: Read>(mut reader: R) -> Result<Self> {
        let code = reader.read_u16::<BigEndian>()?;
        let message_len = reader.read_u32::<BigEndian>()?;
        let mut message_buf = vec![0u8; message_len as usize];
        reader.read_exact(&mut message_buf)?;
        let message = String::from_utf8(message_buf).map_err(|_| IPCError::CorruptedEvent)?;

        Ok(ErrorEventData { code, message })
    }
}
