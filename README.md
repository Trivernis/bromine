# rmp-ipc

Interprocess Communication via TCP using Rust MessagePack.

## Usage

**Client:**
```rust
use rmp_ipc::{callback, Event, context::Context, IPCBuilder, error::Result};

/// Callback ping function
async fn handle_ping(ctx: &Context, event: Event) -> Result<()> {
    println!("Received ping event.");
    ctx.emitter.emit_response(event.id(), "pong", ()).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    // create the client
    let ctx = IPCBuilder::new()
        .address("127.0.0.1:2020")
        // register callback
        .on("ping", callback!(handle_ping))
        .build_client().await.unwrap();

// emit an initial event
    let response = ctx.emitter.emit("ping", ()).await?.await_response(&ctx).await?;
}
```

**Server:**
```rust
use rmp_ipc::{IPCBuilder, callback};
// create the server

#[tokio::main]
async fn main() {
    IPCBuilder::new()
        .address("127.0.0.1:2020")
        // register callback
        .on("ping", callback!(ctx, event, async move {
            println!("Received ping event.");
            Ok(())
        }))
        .build_server().await.unwrap();
}
```

### Namespaces

**Client:**
```rust
use rmp_ipc::IPCBuilder;
// create the client

#[tokio::main]
async fn main() {
    let ctx = IPCBuilder::new()
        .address("127.0.0.1:2020")
        // register namespace
        .namespace("mainspace-client")
        // register callback (without macro)
        .on("ping", |_ctx, _event| Box::pin(async move {
            println!("Received ping event.");
            Ok(())
        }))
        .build()
        .build_client().await.unwrap();

// emit an initial event
    let response = ctx.emitter.emit_to("mainspace-server", "ping", ()).await?
        .await_response(&ctx).await?;
}
```

**Server:**
```rust
use rmp_ipc::{IPCBuilder, EventHandler, namespace, command, Event, context::Context};
// create the server

pub struct MyNamespace;

impl MyNamespace {
     async fn ping(_ctx: &Context, _event: Event) -> Result<()> {
         println!("My namespace received a ping");
         Ok(())
     }
}

impl NamespaceProvider for MyNamespace {
     fn name() -> &'static str {"my_namespace"}
 
     fn register(handler: &mut EventHandler) {
         events!(handler, 
            "ping" => Self::ping
         );
     }
}

#[tokio::main]
async fn main() {
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
        .add_namespace(namespace!(MyNamespace))
        .build_server().await.unwrap();
}
```

## Benchmarks

Benchmarks are generated on each commit. They can be reviewed [here](https://trivernis.github.io/rmp-ipc/report/).

## License

Apache-2.0