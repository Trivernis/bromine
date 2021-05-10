# rmp-ipc

Interprocess Communication via TCP using Rust MessagePack.

## Usage

**Client:**
```rust
use rmp_ipc::IPCBuilder;
// create the client
let emitter = IPCBuilder::new()
    .address("127.0.0.1:2020")
    // register callback
    .on("ping", |_ctx, _event| Box::pin(async move {
        println!("Received ping event.");
        Ok(())
    }))
    .build_client().await.unwrap();

// emit an initial event
emitter.emit("ping", ()).await?;
```

**Server:**
```rust
use rmp_ipc::IPCBuilder;
// create the server
IPCBuilder::new()
    .address("127.0.0.1:2020")
    // register callback
    .on("ping", |_ctx, _event| Box::pin(async move {
        println!("Received ping event.");
        Ok(())
    }))
    .build_server().await.unwrap();
```

## License

Apache-2.0