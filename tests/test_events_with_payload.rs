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
    ctx.emit("ping", payload).await.unwrap();

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
    let reply = ctx.emit("ping", payload).await_reply().await.unwrap();
    let reply_payload = reply.payload::<SimplePayload>().unwrap();

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

async fn handle_ping_event(ctx: &Context, event: Event) -> IPCResult<Response> {
    increment_counter_for_event(ctx, &event).await;
    let payload = event.payload::<SimplePayload>()?;
    ctx.emit("pong", payload).await?;

    Ok(Response::empty())
}

async fn handle_pong_event(ctx: &Context, event: Event) -> IPCResult<Response> {
    increment_counter_for_event(ctx, &event).await;
    let _payload = event.payload::<SimplePayload>()?;

    Ok(Response::empty())
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
    use bromine::context::Context;
    use bromine::error::Result;
    use bromine::payload::{FromPayload, IntoPayload};
    use bromine::prelude::IPCResult;
    use byteorder::{BigEndian, ReadBytesExt};
    use bytes::{BufMut, Bytes, BytesMut};
    use std::io::Read;

    pub struct SimplePayload {
        pub string: String,
        pub number: u32,
    }

    impl IntoPayload for SimplePayload {
        fn into_payload(self, _: &Context) -> IPCResult<Bytes> {
            let mut buf = BytesMut::new();
            buf.put_u16(self.string.len() as u16);
            buf.put(Bytes::from(self.string));
            buf.put_u32(self.number);

            Ok(buf.freeze())
        }
    }

    impl FromPayload for SimplePayload {
        fn from_payload<R: Read>(mut reader: R) -> Result<Self> {
            let string_length = reader.read_u16::<BigEndian>()?;
            let mut string_buf = vec![0u8; string_length as usize];
            reader.read_exact(&mut string_buf)?;
            let string = String::from_utf8(string_buf).unwrap();
            let number = reader.read_u32::<BigEndian>()?;

            Ok(Self { string, number })
        }
    }
}
