mod utils;

use crate::utils::start_server_and_client;
use bromine::prelude::*;
use payload_impl::SimplePayload;
use std::time::Duration;
use utils::call_counter::*;
use utils::get_free_port;
use utils::protocol::*;

#[tokio::test]
async fn it_sends_payloads() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let payload = SimplePayload {
        number: 0,
        string: String::from("Hello World"),
    };
    #[cfg(feature = "serialize")]
    let payload = payload.into_serde_payload(&ctx);

    ctx.emitter.emit("ping", payload).await.unwrap();

    // wait for the event to be handled
    tokio::time::sleep(Duration::from_millis(10)).await;

    let counters = get_counter_from_context(&ctx).await;

    assert_eq!(counters.get("ping").await, 1);
    assert_eq!(counters.get("pong").await, 1);
}

#[tokio::test]
async fn it_receives_payloads() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let payload = SimplePayload {
        number: 0,
        string: String::from("Hello World"),
    };
    #[cfg(feature = "serialize")]
    let payload = payload.into_serde_payload(&ctx);

    let reply = ctx
        .emitter
        .emit("ping", payload)
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    #[cfg(not(feature = "serialize"))]
    let reply_payload = reply.payload::<SimplePayload>().unwrap();

    #[cfg(feature = "serialize")]
    let reply_payload = reply.serde_payload::<SimplePayload>().unwrap();

    let counters = get_counter_from_context(&ctx).await;

    assert_eq!(counters.get("ping").await, 1);
    assert_eq!(reply_payload.string, String::from("Hello World"));
    assert_eq!(reply_payload.number, 0);
}

async fn get_client_with_server(port: u8) -> Context {
    start_server_and_client(move || get_builder(port)).await
}

fn get_builder(port: u8) -> IPCBuilder<TestProtocolListener> {
    IPCBuilder::new()
        .address(port)
        .on("ping", callback!(handle_ping_event))
        .on("pong", callback!(handle_pong_event))
        .timeout(Duration::from_millis(10))
}

async fn handle_ping_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;
    let payload = get_simple_payload(&event)?;
    #[cfg(feature = "serialize")]
    {
        ctx.emitter
            .emit_response(event.id(), "pong", payload.into_serde_payload(&ctx))
            .await?;
    }
    #[cfg(not(feature = "serialize"))]
    {
        ctx.emitter
            .emit_response(event.id(), "pong", payload)
            .await?;
    }

    Ok(())
}

async fn handle_pong_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;
    let _payload = get_simple_payload(&event)?;

    Ok(())
}

fn get_simple_payload(event: &Event) -> IPCResult<SimplePayload> {
    #[cfg(feature = "serialize")]
    {
        event.serde_payload::<SimplePayload>()
    }
    #[cfg(not(feature = "serialize"))]
    {
        event.payload::<SimplePayload>()
    }
}

#[cfg(feature = "serialize")]
mod payload_impl {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    pub struct SimplePayload {
        pub string: String,
        pub number: u32,
    }
}

#[cfg(not(feature = "serialize"))]
mod payload_impl {
    use bromine::error::Result;
    use bromine::payload::{EventReceivePayload, EventSendPayload};
    use bromine::prelude::IPCResult;
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::Read;

    pub struct SimplePayload {
        pub string: String,
        pub number: u32,
    }

    impl EventSendPayload for SimplePayload {
        fn to_payload_bytes(self) -> IPCResult<Vec<u8>> {
            let mut buf = Vec::new();
            let string_length = self.string.len() as u16;
            let string_length_bytes = string_length.to_be_bytes();
            buf.append(&mut string_length_bytes.to_vec());
            let mut string_bytes = self.string.into_bytes();
            buf.append(&mut string_bytes);
            let num_bytes = self.number.to_be_bytes();
            buf.append(&mut num_bytes.to_vec());

            Ok(buf)
        }
    }

    impl EventReceivePayload for SimplePayload {
        fn from_payload_bytes<R: Read>(mut reader: R) -> Result<Self> {
            let string_length = reader.read_u16::<BigEndian>()?;
            let mut string_buf = vec![0u8; string_length as usize];
            reader.read_exact(&mut string_buf)?;
            let string = String::from_utf8(string_buf).unwrap();
            let number = reader.read_u32::<BigEndian>()?;

            Ok(Self { string, number })
        }
    }
}
