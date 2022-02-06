#[cfg(feature = "serialize")]
use bromine::prelude::*;
#[cfg(feature = "serialize")]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "serialize_rmp")]
#[test]
fn it_serializes_messagepack() {
    test_serialization::<BigTestPayload>(DynamicSerializer::Messagepack).unwrap();
    test_serialization::<AdjacentlyTaggedEnum>(DynamicSerializer::Messagepack).unwrap();
    test_serialization::<u128>(DynamicSerializer::Messagepack).unwrap_err();
}

#[cfg(feature = "serialize_bincode")]
#[test]
fn it_serializes_bincode() {
    test_serialization::<BigTestPayload>(DynamicSerializer::Bincode).unwrap();
    test_serialization::<AdjacentlyTaggedEnum>(DynamicSerializer::Bincode).unwrap_err();
    test_serialization::<u128>(DynamicSerializer::Bincode).unwrap();
}

#[cfg(feature = "serialize_postcard")]
#[test]
fn it_serializes_postcard() {
    test_serialization::<BigTestPayload>(DynamicSerializer::Postcard).unwrap();
    test_serialization::<AdjacentlyTaggedEnum>(DynamicSerializer::Postcard).unwrap_err();
    test_serialization::<u128>(DynamicSerializer::Postcard).unwrap();
}

#[cfg(feature = "serialize_json")]
#[test]
fn it_serializes_json() {
    test_serialization::<BigTestPayload>(DynamicSerializer::Json).unwrap();
    test_serialization::<AdjacentlyTaggedEnum>(DynamicSerializer::Json).unwrap();
    test_serialization::<u128>(DynamicSerializer::Json).unwrap();
}

#[cfg(feature = "serialize")]
fn test_serialization<D: Default + Serialize + DeserializeOwned + Clone + Eq + Debug>(
    serializer: DynamicSerializer,
) -> IPCResult<()> {
    let test_payload = SerdePayload::new(serializer, D::default());
    let payload_bytes = test_payload.clone().try_into_bytes()?;
    let payload = SerdePayload::<D>::from_payload(&payload_bytes[..])?;

    assert_eq!(payload.data(), test_payload.data());

    Ok(())
}

#[cfg(feature = "serialize")]
pub mod payload {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
    pub struct BigTestPayload {
        items: Vec<u64>,
        variant: TestPayloadEnum,
        string: String,
        signed: i32,
        maps: HashMap<String, i64>,
    }

    #[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
    pub enum TestPayloadEnum {
        First,
        Second,
        Third(usize),
    }

    #[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
    #[serde(tag = "variant", content = "data")]
    pub enum AdjacentlyTaggedEnum {
        Variant1(u64),
        Variant2(String),
        Variant3(Vec<u8>),
    }

    impl Default for AdjacentlyTaggedEnum {
        fn default() -> Self {
            Self::Variant3(vec![0, 1, 2])
        }
    }

    impl Default for BigTestPayload {
        fn default() -> Self {
            let mut maps = HashMap::new();
            maps.insert("Hello".to_string(), 12);
            maps.insert("Wäüörld".to_string(), -12380);

            BigTestPayload {
                items: vec![0u64, 12452u64, u64::MAX],
                variant: TestPayloadEnum::Third(12),
                string: String::from("Hello World ſð"),
                signed: -12,
                maps,
            }
        }
    }
}
#[cfg(feature = "serialize")]
pub use payload::*;
