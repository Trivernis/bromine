use self::super::utils::PingEventData;
use crate::IPCBuilder;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[tokio::test]
async fn it_receives_events() {
    let ctr = Arc::new(AtomicU8::new(0));
    let builder = IPCBuilder::new()
        .on("ping", {
            let ctr = Arc::clone(&ctr);
            move |ctx, e| {
                let ctr = Arc::clone(&ctr);
                Box::pin(async move {
                    ctr.fetch_add(1, Ordering::Relaxed);
                    let mut ping_data = e.data::<PingEventData>()?;
                    ping_data.time = SystemTime::now();
                    ping_data.ttl -= 1;

                    if ping_data.ttl > 0 {
                        ctx.emitter.emit("ping", ping_data).await?;
                    }

                    Ok(())
                })
            }
        })
        .address("127.0.0.1:8282");
    let server_running = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let server_running = Arc::clone(&server_running);
        let builder = builder.clone();
        async move {
            server_running.store(true, Ordering::SeqCst);
            builder.build_server().await.unwrap();
        }
    });
    while !server_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let client = builder.build_client().await.unwrap();
    client
        .emit(
            "ping",
            PingEventData {
                ttl: 16,
                time: SystemTime::now(),
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(ctr.load(Ordering::SeqCst), 16);
}
