# rmp-ipc

Interprocess Communication via TCP using Rust MessagePack.

## Usage

**Client:**
```rust
use rmp_ipc::IPCBuilder;
// create the client
let ctx = IPCBuilder::new()
    .address("127.0.0.1:2020")
    // register callback
    .on("ping", |_ctx, _event| Box::pin(async move {
        println!("Received ping event.");
        Ok(())
    }))
    .build_client().await.unwrap();

// emit an initial event
let response = ctx.emitter.emit("ping", ()).await?.await_response(&ctx).await?;
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

### Namespaces

**Client:**
```rust
use rmp_ipc::IPCBuilder;
// create the client
let ctx = IPCBuilder::new()
    .address("127.0.0.1:2020")
    // register namespace
    .namespace("mainspace-client")
    // register callback
    .on("ping", |_ctx, _event| Box::pin(async move {
        println!("Received ping event.");
        Ok(())
    }))
    .build()
    .build_client().await.unwrap();

// emit an initial event
let response = ctx.emitter.emit_to("mainspace-server", "ping", ()).await?
    .await_response(&ctx).await?;
```

**Server:**
```rust
use rmp_ipc::IPCBuilder;
// create the server
IPCBuilder::new()
    .address("127.0.0.1:2020")
    // register namespace
    .namespace("mainspace-server")
    // register callback
    .on("ping", |_ctx, _event| Box::pin(async move {
        println!("Received ping event.");
        Ok(())
    }))
    .build()
    .build_server().await.unwrap();
```

## License

Apache-2.0