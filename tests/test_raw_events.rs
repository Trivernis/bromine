mod test_protocol;

use bromine::prelude::*;
use std::time::Duration;
use test_protocol::*;

async fn handle_ping_event(ctx: &Context, event: Event) -> IPCResult<()> {
    ctx.emitter.emit_response(event.id(), "pong", ()).await?;

    Ok(())
}

async fn handle_pong_event(_ctx: &Context, _event: Event) -> IPCResult<()> {
    Ok(())
}

fn get_builder(port: u8) -> IPCBuilder<TestProtocolListener> {
    IPCBuilder::new()
        .address(port)
        .on(
            "ping",
            callback!(
                ctx,
                event,
                async move { handle_ping_event(ctx, event).await }
            ),
        )
        .timeout(Duration::from_millis(100))
        .on(
            "pong",
            callback!(
                ctx,
                event,
                async move { handle_pong_event(ctx, event).await }
            ),
        )
}

#[tokio::test]
async fn it_passes_events() {
    tokio::task::spawn(async { get_builder(0).build_server().await.unwrap() });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ctx = get_builder(0).build_client().await.unwrap();
    ctx.emitter
        .emit("ping", ())
        .await
        .unwrap()
        .await_reply(&ctx)
        .await
        .unwrap();
}
