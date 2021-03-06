<h1 align="center">
bromine
</h1>
<p align="center">
    <a href="https://crates.io/crates/bromine">
        <img src="https://img.shields.io/crates/v/bromine?style=for-the-badge">
    </a>
    <a href="https://docs.rs/bromine">
        <img src="https://img.shields.io/docsrs/bromine?style=for-the-badge">
    </a>
</p>
<p align="center">
Asynchronous event driven interprocess communication supporting tcp and unix domain sockets.
</p>

- - -

## Usage

**Client:**

```rust
use bromine::prelude::*;
use tokio::net::TcpListener;

/// Callback ping function
async fn handle_ping(ctx: &Context, event: Event) -> Result<()> {
    println!("Received ping event.");
    ctx.emit("pong", ()).await?;
    Ok(Response::empty())
}

#[tokio::main]
async fn main() {
    // create the client
    let ctx = IPCBuilder::<TcpListener>::new()
        .address("127.0.0.1:2020")
        // register callback
        .on("ping", callback!(handle_ping))
        .build_client().await.unwrap();

    // emit an event and wait for responses
    let response = ctx.emit("ping", ()).await_reply().await?;
    
    // emit an event and get all responses as stream
    let stream = ctx.emit("ping", ()).stream_replies().await?;
    
    while let Some(Ok(event)) = stream.next().await {
        println!("{}", event.name());
    }
}
```

**Server:**

```rust
use bromine::prelude::*;
use tokio::net::TcpListener;
// create the server

#[tokio::main]
async fn main() {
    IPCBuilder::<TcpListener>::new()
        .address("127.0.0.1:2020")
        // register callback
        .on("ping", callback!(ctx, event, async move {
            println!("Received ping event.");
            for _ in 0..10 {
                ctx.emit("pong", ()).await?;
            }
            Ok(Response::empty())
        }))
        .build_server().await.unwrap();
}
```

### Namespaces

**Client:**

```rust
use bromine::prelude::*;
use tokio::net::TcpListener;
// create the client

#[tokio::main]
async fn main() {
    let ctx = IPCBuilder::<TcpListener>::new()
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
    let response = ctx.emit_to("mainspace-server", "ping", ())
        .await_response().await?;
}
```

**Server:**

```rust
use bromine::prelude::*;
use tokio::net::TcpListener;
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
    IPCBuilder::<TcpListener>::new()
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

## License

Apache-2.0