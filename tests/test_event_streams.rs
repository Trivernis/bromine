use crate::utils::call_counter::{get_counter_from_context, increment_counter_for_event};
use crate::utils::protocol::TestProtocolListener;
use crate::utils::{get_free_port, start_server_and_client};
use bromine::prelude::*;
use byteorder::ReadBytesExt;
use bytes::Bytes;
use futures::StreamExt;
use std::io::Read;
use std::time::Duration;

mod utils;

/// When awaiting the reply to an event the handler for the event doesn't get called.
/// Therefore we expect it to have a call count of 0.
#[tokio::test]
async fn it_receives_responses() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let mut reply_stream = ctx
        .emit("stream", EmptyPayload)
        .stream_replies()
        .await
        .unwrap();

    let mut reply_stream_2 = ctx
        .emit("stream", EmptyPayload)
        .stream_replies()
        .await
        .unwrap();

    for i in 0u8..=100 {
        if let Some(result) = reply_stream.next().await {
            let event = result.unwrap();
            assert_eq!(event.payload::<NumberPayload>().unwrap().0, i)
        } else {
            panic!("stream 1 has no value {}", i);
        }
        if let Some(result) = reply_stream_2.next().await {
            let event = result.unwrap();
            assert_eq!(event.payload::<NumberPayload>().unwrap().0, i)
        } else {
            panic!("stream 2 has no value {}", i);
        }
    }
    let counter = get_counter_from_context(&ctx).await;
    assert_eq!(counter.get("stream").await, 2);
}

async fn get_client_with_server(port: u8) -> Context {
    start_server_and_client(move || get_builder(port)).await
}

fn get_builder(port: u8) -> IPCBuilder<TestProtocolListener> {
    IPCBuilder::new()
        .address(port)
        .timeout(Duration::from_millis(100))
        .on("stream", callback!(handle_stream_event))
}

async fn handle_stream_event(ctx: &Context, event: Event) -> IPCResult<Response> {
    increment_counter_for_event(ctx, &event).await;
    for i in 0u8..=99 {
        ctx.emit("number", NumberPayload(i)).await?;
    }

    ctx.response(NumberPayload(100))
}

pub struct EmptyPayload;

impl IntoPayload for EmptyPayload {
    fn into_payload(self, _: &Context) -> IPCResult<Bytes> {
        Ok(Bytes::new())
    }
}

pub struct NumberPayload(u8);

impl IntoPayload for NumberPayload {
    fn into_payload(self, _: &Context) -> IPCResult<Bytes> {
        Ok(Bytes::from(vec![self.0]))
    }
}

impl FromPayload for NumberPayload {
    fn from_payload<R: Read>(mut reader: R) -> IPCResult<Self> {
        let num = reader.read_u8()?;

        Ok(NumberPayload(num))
    }
}
