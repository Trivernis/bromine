use crate::prelude::IPCResult;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Read;

#[cfg(feature = "serialize")]
pub use super::payload_serializer::*;

/// Trait that serializes a type into bytes and can fail
pub trait TryIntoBytes {
    fn try_into_bytes(self) -> IPCResult<Vec<u8>>;
}

/// Trait that serializes a type into bytes and never fails
pub trait IntoBytes {
    fn into_bytes(self) -> Vec<u8>;
}

/// Trait to convert event data into sending bytes
/// It is implemented for all types that implement Serialize
pub trait IntoPayload {
    fn into_payload(self, ctx: &Context) -> IPCResult<Vec<u8>>;
}

/// Trait to get the event data from receiving bytes.
/// It is implemented for all types that are DeserializeOwned
pub trait FromPayload: Sized {
    fn from_payload<R: Read>(reader: R) -> IPCResult<Self>;
}

/// A payload wrapper type for sending bytes directly without
/// serializing them
#[derive(Clone)]
pub struct BytePayload {
    bytes: Vec<u8>,
}

impl BytePayload {
    #[inline]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns the bytes of the payload
    #[inline]
    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl IntoPayload for BytePayload {
    #[inline]
    fn into_payload(self, _: &Context) -> IPCResult<Vec<u8>> {
        Ok(self.bytes)
    }
}

impl FromPayload for BytePayload {
    #[inline]
    fn from_payload<R: Read>(mut reader: R) -> IPCResult<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;

        Ok(Self::new(buf))
    }
}

/// A payload wrapper that allows storing two different payloads
/// independent from each other. For example one payload can be
/// a payload serialized by serde while the other is a raw byte
/// payload
pub struct TandemPayload<P1, P2> {
    load1: P1,
    load2: P2,
}

impl<P1, P2> TandemPayload<P1, P2> {
    #[inline]
    pub fn new(load1: P1, load2: P2) -> Self {
        Self { load1, load2 }
    }

    /// Returns both payload stored in the tandem payload
    #[inline]
    pub fn into_inner(self) -> (P1, P2) {
        (self.load1, self.load2)
    }
}

impl<P1: IntoPayload, P2: IntoPayload> IntoPayload for TandemPayload<P1, P2> {
    fn into_payload(self, ctx: &Context) -> IPCResult<Vec<u8>> {
        let mut p1_bytes = self.load1.into_payload(&ctx)?;
        let mut p2_bytes = self.load2.into_payload(&ctx)?;

        let mut p1_length_bytes = (p1_bytes.len() as u64).to_be_bytes().to_vec();
        let mut p2_length_bytes = (p2_bytes.len() as u64).to_be_bytes().to_vec();

        let mut bytes = Vec::new();
        bytes.append(&mut p1_length_bytes);
        bytes.append(&mut p1_bytes);
        bytes.append(&mut p2_length_bytes);
        bytes.append(&mut p2_bytes);

        Ok(bytes)
    }
}

impl<P1: FromPayload, P2: FromPayload> FromPayload for TandemPayload<P1, P2> {
    fn from_payload<R: Read>(mut reader: R) -> IPCResult<Self> {
        let p1_length = reader.read_u64::<BigEndian>()?;
        let mut load1_bytes = vec![0u8; p1_length as usize];
        reader.read_exact(&mut load1_bytes)?;

        let p2_length = reader.read_u64::<BigEndian>()?;
        let mut load2_bytes = vec![0u8; p2_length as usize];
        reader.read_exact(&mut load2_bytes)?;

        Ok(Self {
            load1: P1::from_payload(load1_bytes.as_slice())?,
            load2: P2::from_payload(load2_bytes.as_slice())?,
        })
    }
}

#[cfg(not(feature = "serialize"))]
impl IntoPayload for () {
    fn into_payload(self, _: &Context) -> IPCResult<Vec<u8>> {
        Ok(vec![])
    }
}

#[cfg(feature = "serialize")]
mod serde_payload {
    use super::DynamicSerializer;
    use crate::context::Context;
    use crate::payload::{FromPayload, TryIntoBytes};
    use crate::prelude::{IPCResult, IntoPayload};
    use byteorder::ReadBytesExt;
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::io::Read;

    /// A payload representing a payload storing serde serialized data
    pub struct SerdePayload<T> {
        data: T,
        serializer: DynamicSerializer,
    }

    impl<T> SerdePayload<T> {
        /// Creates a new serde payload with a specified serializer
        #[inline]
        pub fn new(serializer: DynamicSerializer, data: T) -> Self {
            Self { serializer, data }
        }

        #[inline]
        pub fn data(self) -> T {
            self.data
        }
    }

    impl<T: Clone> Clone for SerdePayload<T> {
        fn clone(&self) -> Self {
            Self {
                serializer: self.serializer.clone(),
                data: self.data.clone(),
            }
        }
    }

    impl<T: Serialize> TryIntoBytes for SerdePayload<T> {
        fn try_into_bytes(self) -> IPCResult<Vec<u8>> {
            let mut buf = Vec::new();
            let mut data_bytes = self.serializer.serialize(self.data)?;
            let format_id = self.serializer as u8;
            buf.push(format_id);
            buf.append(&mut data_bytes);

            Ok(buf)
        }
    }

    impl<T: Serialize> IntoPayload for SerdePayload<T> {
        #[inline]
        fn into_payload(self, _: &Context) -> IPCResult<Vec<u8>> {
            self.try_into_bytes()
        }
    }

    impl<T: DeserializeOwned> FromPayload for SerdePayload<T> {
        fn from_payload<R: Read>(mut reader: R) -> IPCResult<Self> {
            let format_id = reader.read_u8()?;
            let serializer = DynamicSerializer::from_primitive(format_id as usize)?;
            let data = serializer.deserialize(reader)?;

            Ok(Self { serializer, data })
        }
    }

    impl<T: Serialize> IntoPayload for T {
        #[inline]
        fn into_payload(self, ctx: &Context) -> IPCResult<Vec<u8>> {
            ctx.create_serde_payload(self).into_payload(&ctx)
        }
    }

    impl<T: DeserializeOwned> FromPayload for T {
        #[inline]
        fn from_payload<R: Read>(reader: R) -> IPCResult<Self> {
            let serde_payload = SerdePayload::<Self>::from_payload(reader)?;

            Ok(serde_payload.data)
        }
    }
}

use crate::context::Context;
#[cfg(feature = "serialize")]
pub use serde_payload::*;
