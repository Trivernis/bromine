mod call_counter;
mod protocol;
mod utils;

use bromine::prelude::*;
use call_counter::*;
use protocol::*;
use std::time::Duration;
use tokio::sync::oneshot::channel;
use utils::get_free_port;

/// Simple events are passed from the client to the server and responses
/// are emitted back to the client. Both will have received an event.
#[tokio::test]
async fn it_sends_events() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    ctx.emitter.emit("ping", ()).await.unwrap();

    // allow the event to be processed
    tokio::time::sleep(Duration::from_millis(10)).await;
    let counter = get_counter_from_context(&ctx).await;

    assert_eq!(counter.get("ping").await, 1);
    assert_eq!(counter.get("pong").await, 1);
}

/// When awaiting the reply to an event the handler for the event doesn't get called.
/// Therefore we expect it to have a call count of 0.
#[tokio::test]
async fn it_receives_responses() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let reply = ctx
        .emitter
        .emit("ping", ())
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
    let counter = get_counter_from_context(&ctx).await;

    assert_eq!(reply.name(), "pong");
    assert_eq!(counter.get("ping").await, 1);
    assert_eq!(counter.get("pong").await, 0);
}

/// When emitting errors from handlers the client should receive an error event
/// with the error that occurred on the server.
#[tokio::test]
async fn it_handles_errors() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    ctx.emitter.emit("create_error", ()).await.unwrap();
    // allow the event to be processed
    tokio::time::sleep(Duration::from_millis(10)).await;
    let counter = get_counter_from_context(&ctx).await;

    assert_eq!(counter.get("error").await, 1);
}

/// When waiting for the reply to an event and an error occurs, the error should
/// bypass the handler and be passed as the Err variant on the await reply instead.
#[tokio::test]
async fn it_receives_error_responses() {
    let port = get_free_port();
    let ctx = get_client_with_server(port).await;
    let result = ctx
        .emitter
        .emit("create_error", ())
        .await
        .unwrap()
        .await_reply(&ctx)
        .await;

    let counter = get_counter_from_context(&ctx).await;

    assert!(result.is_err());
    assert_eq!(counter.get("error").await, 0);
}

async fn get_client_with_server(port: u8) -> Context {
    let counters = CallCounter::default();
    let (sender, receiver) = channel::<()>();

    tokio::task::spawn({
        let counters = counters.clone();

        async move {
            sender.send(()).unwrap();
            get_builder(port)
                .insert::<CallCounterKey>(counters)
                .build_server()
                .await
                .unwrap()
        }
    });
    receiver.await.unwrap();

    get_builder(port)
        .insert::<CallCounterKey>(counters)
        .build_client()
        .await
        .unwrap()
}

fn get_builder(port: u8) -> IPCBuilder<TestProtocolListener> {
    IPCBuilder::new()
        .address(port)
        .timeout(Duration::from_millis(100))
        .on("ping", callback!(handle_ping_event))
        .on("pong", callback!(handle_pong_event))
        .on("create_error", callback!(handle_create_error_event))
        .on("error", callback!(handle_error_event))
}

async fn handle_ping_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;
    ctx.emitter.emit_response(event.id(), "pong", ()).await?;

    Ok(())
}

async fn handle_pong_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;

    Ok(())
}

async fn handle_create_error_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;

    Err(IPCError::from("Test Error"))
}

async fn handle_error_event(ctx: &Context, event: Event) -> IPCResult<()> {
    increment_counter_for_event(ctx, &event).await;

    Ok(())
}
