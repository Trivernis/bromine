#![cfg(feature = "encryption_layer")]

use crate::utils::call_counter::increment_counter_for_event;
use crate::utils::protocol::TestProtocolListener;
use crate::utils::{get_free_port, start_server_and_client};
use bromine::prelude::encrypted::{EncryptedListener, EncryptionOptions, Keys};
use bromine::prelude::*;
use bromine::utils::generate_secret;
use bromine::IPCBuilder;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{BufMut, Bytes, BytesMut};
use dashmap::DashMap;
use futures::StreamExt;
use lazy_static::lazy_static;
use rand_core::RngCore;
use std::io::Read;
use std::time::Duration;
use x25519_dalek::{PublicKey, StaticSecret};

mod utils;

pub fn get_secret<S: AsRef<str>>(name: S) -> StaticSecret {
    lazy_static! {
        static ref KEYS: DashMap<String, StaticSecret> = DashMap::new();
    }
    if KEYS.contains_key(name.as_ref()) {
        KEYS.get(name.as_ref()).as_ref().unwrap().value().clone()
    } else {
        let secret = generate_secret();
        KEYS.insert(name.as_ref().to_string(), secret.clone());

        secret
    }
}

#[tokio::test]
async fn it_sends_and_receives_smaller_packages() {
    send_and_receive_bytes(140).await.unwrap();
}

#[tokio::test]
async fn it_sends_and_receives_larger_packages() {
    send_and_receive_bytes(1024 * 32).await.unwrap();
}

#[tokio::test]
async fn it_sends_and_receives_strings() {
    let ctx = get_client_with_server().await;
    let response = ctx
        .emit("string", StringPayload(String::from("Hello World")))
        .await_reply()
        .await
        .unwrap();
    let response_string = response.payload::<StringPayload>().unwrap().0;

    assert_eq!(&response_string, "Hello World")
}

async fn send_and_receive_bytes(byte_size: usize) -> IPCResult<()> {
    let ctx = get_client_with_server().await;
    let mut rng = rand::thread_rng();
    let mut buffer = vec![0u8; byte_size];
    rng.fill_bytes(&mut buffer);

    let mut stream = ctx
        .emit("bytes", BytePayload::new(buffer.clone()))
        .stream_replies()
        .await?;
    let mut count = 0;

    while let Some(response) = stream.next().await {
        let bytes = response.unwrap().payload::<BytePayload>()?;
        assert_eq!(bytes.into_inner(), buffer);
        count += 1;
    }
    assert_eq!(count, 100);

    Ok(())
}

async fn get_client_with_server() -> Context {
    let port = get_free_port();

    start_server_and_client(move || get_builder(port)).await
}

fn get_builder(port: u8) -> IPCBuilder<EncryptedListener<TestProtocolListener>> {
    let server_secret = get_secret(format!("server-{}", port));
    let client_secret = get_secret(format!("client-{}", port));
    let client_keys = Keys {
        secret: client_secret.clone(),
        known_peers: vec![PublicKey::from(&server_secret)],
        allow_unknown: false,
    };
    let server_keys = Keys {
        secret: server_secret.clone(),
        known_peers: vec![PublicKey::from(&client_secret)],
        allow_unknown: false,
    };
    IPCBuilder::new()
        .client_options(EncryptionOptions {
            keys: client_keys,
            inner_options: (),
        })
        .server_options(EncryptionOptions {
            keys: server_keys,
            inner_options: (),
        })
        .address(port)
        .on("bytes", callback!(handle_bytes))
        .on("string", callback!(handle_string))
        .timeout(Duration::from_secs(10))
}

async fn handle_bytes(ctx: &Context, event: Event) -> IPCResult<Response> {
    increment_counter_for_event(ctx, &event).await;
    let bytes = event.payload::<BytePayload>()?.into_inner();

    for _ in 0u8..99 {
        ctx.emit("bytes", BytePayload::new(bytes.clone())).await?;
    }

    ctx.response(BytePayload::new(bytes))
}

async fn handle_string(ctx: &Context, event: Event) -> IPCResult<Response> {
    ctx.response(event.payload::<StringPayload>()?)
}

pub struct StringPayload(String);

impl IntoPayload for StringPayload {
    fn into_payload(self, _: &Context) -> IPCResult<Bytes> {
        let mut buf = BytesMut::with_capacity(self.0.len() + 4);
        buf.put_u32(self.0.len() as u32);
        buf.put(Bytes::from(self.0));

        Ok(buf.freeze())
    }
}

impl FromPayload for StringPayload {
    fn from_payload<R: Read>(mut reader: R) -> IPCResult<Self> {
        let len = reader.read_u32::<BigEndian>()?;
        let mut buf = vec![0u8; len as usize];
        reader.read_exact(&mut buf)?;
        let string = String::from_utf8(buf).map_err(|_| IPCError::from("not a string"))?;

        Ok(StringPayload(string))
    }
}
