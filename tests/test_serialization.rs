#[cfg(feature = "serialize")]
use bromine::prelude::*;

#[cfg(feature = "serialize_rmp")]
#[test]
fn it_serializes_messagepack() {
    test_serialization(DynamicSerializer::Messagepack)
}

#[cfg(feature = "serialize_bincode")]
#[test]
fn it_serializes_bincode() {
    test_serialization(DynamicSerializer::Bincode)
}

#[cfg(feature = "serialize_postcard")]
#[test]
fn it_serializes_postcard() {
    test_serialization(DynamicSerializer::Postcard)
}

#[cfg(feature = "serialize_json")]
#[test]
fn it_serializes_json() {
    test_serialization(DynamicSerializer::Json)
}

#[cfg(feature = "serialize")]
fn test_serialization(serializer: DynamicSerializer) {
    let test_payload = get_test_payload(serializer);
    let payload_bytes = test_payload.clone().try_into_bytes().unwrap();
    let payload = TestSerdePayload::from_payload(&payload_bytes[..]).unwrap();
    assert_eq!(payload.data(), test_payload.data())
}

#[cfg(feature = "serialize")]
pub mod payload {
    use bromine::payload::{DynamicSerializer, SerdePayload};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    pub type TestSerdePayload = SerdePayload<TestPayload>;

    #[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
    pub struct TestPayload {
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

    pub fn get_test_payload(serializer: DynamicSerializer) -> SerdePayload<TestPayload> {
        let mut maps = HashMap::new();
        maps.insert("Hello".to_string(), 12);

        maps.insert("Wäüörld".to_string(), -12380);
        let inner_payload = TestPayload {
            items: vec![0u64, 12452u64, u64::MAX],
            variant: TestPayloadEnum::Third(12),
            string: String::from("Hello World ſð"),
            signed: -12,
            maps,
        };

        SerdePayload::new(serializer, inner_payload)
    }
}
#[cfg(feature = "serialize")]
pub use payload::*;
