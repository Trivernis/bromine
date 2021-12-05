use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Read;
use thiserror::Error;

#[cfg(feature = "serialize_rmp")]
mod serialize_rmp;

#[cfg(feature = "serialize_bincode")]
mod serialize_bincode;

#[cfg(feature = "serialize_postcard")]
mod serialize_postcard;

#[cfg(feature = "serialize_json")]
mod serialize_json;

pub type SerializationResult<T> = std::result::Result<T, SerializationError>;

#[derive(Debug, Error)]
pub enum SerializationError {
    #[cfg(feature = "serialize_rmp")]
    #[error("failed to serialize messagepack payload: {0}")]
    SerializeRmp(#[from] rmp_serde::encode::Error),

    #[cfg(feature = "serialize_rmp")]
    #[error("failed to deserialize messagepack payload: {0}")]
    DeserializeRmp(#[from] rmp_serde::decode::Error),

    #[cfg(feature = "serialize_bincode")]
    #[error("failed to de/serialize bincode payload: {0}")]
    Bincode(#[from] bincode::Error),

    #[cfg(feature = "serialize_postcard")]
    #[error("failed to de/serialize postcard payload: {0}")]
    Postcard(#[from] postcard::Error),

    #[cfg(feature = "serialize_json")]
    #[error("failed to de/serialize json payload: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error occurred on de/serialization: {0}")]
    Io(#[from] std::io::Error),

    #[error("the format {0:?} is not available")]
    UnavailableFormat(DynamicSerializer),

    #[error("tried to create serializer for unknown format {0}")]
    UnknownFormat(usize),
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum DynamicSerializer {
    Messagepack,
    Bincode,
    Postcard,
    Json,
}

impl DynamicSerializer {
    pub fn first_available() -> Self {
        #[cfg(feature = "serialize_rmp")]
        {
            Self::Messagepack
        }

        #[cfg(all(feature = "serialize_bincode", not(feature = "serialize_rmp")))]
        {
            Self::Bincode
        }

        #[cfg(all(
            feature = "serialize_postcard",
            not(any(feature = "serialize_rmp", feature = "serialize_bincode"))
        ))]
        {
            Self::Postcard
        }

        #[cfg(all(
            feature = "serialize_json",
            not(any(
                feature = "serialize_rmp",
                feature = "serialize_bincode",
                feature = "serialize_postcard"
            ))
        ))]
        {
            Self::Json
        }
    }

    pub fn from_primitive(num: usize) -> SerializationResult<Self> {
        match num {
            #[cfg(feature = "serialize_rmp")]
            0 => Ok(Self::Messagepack),

            #[cfg(feature = "serialize_bincode")]
            1 => Ok(Self::Bincode),

            #[cfg(feature = "serialize_postcard")]
            2 => Ok(Self::Postcard),

            #[cfg(feature = "serialize_json")]
            3 => Ok(Self::Json),

            n => Err(SerializationError::UnknownFormat(n)),
        }
    }

    pub fn serialize<T: Serialize>(&self, data: T) -> SerializationResult<Vec<u8>> {
        match self {
            #[cfg(feature = "serialize_rmp")]
            DynamicSerializer::Messagepack => serialize_rmp::serialize(data),

            #[cfg(feature = "serialize_bincode")]
            DynamicSerializer::Bincode => serialize_bincode::serialize(data),

            #[cfg(feature = "serialize_postcard")]
            DynamicSerializer::Postcard => serialize_postcard::serialize(data),

            #[cfg(feature = "serialize_json")]
            DynamicSerializer::Json => serialize_json::serialize(data),

            #[cfg(not(all(
                feature = "serialize_rmp",
                feature = "serialize_bincode",
                feature = "serialize_postcard",
                feature = "serialize_json"
            )))]
            _ => Err(SerializationError::UnavailableFormat(self.clone())),
        }
    }

    pub fn deserialize<T: DeserializeOwned, R: Read>(&self, reader: R) -> SerializationResult<T> {
        match self {
            #[cfg(feature = "serialize_rmp")]
            DynamicSerializer::Messagepack => serialize_rmp::deserialize(reader),

            #[cfg(feature = "serialize_bincode")]
            DynamicSerializer::Bincode => serialize_bincode::deserialize(reader),

            #[cfg(feature = "serialize_postcard")]
            DynamicSerializer::Postcard => serialize_postcard::deserialize(reader),

            #[cfg(feature = "serialize_json")]
            DynamicSerializer::Json => serialize_json::deserialize(reader),

            #[cfg(not(all(
                feature = "serialize_rmp",
                feature = "serialize_bincode",
                feature = "serialize_postcard",
                feature = "serialize_json"
            )))]
            _ => Err(SerializationError::UnavailableFormat(self.clone())),
        }
    }
}
