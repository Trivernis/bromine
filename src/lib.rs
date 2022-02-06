//! This project provides an ipc server and client implementation using
//! messagepack. All calls are asynchronous and event based.
//! Client Example:
//! ```no_run
//! use bromine::prelude::*;
//! use tokio::net::TcpListener;
//!
//! /// Callback ping function
//! async fn handle_ping(ctx: &Context, event: Event) -> IPCResult<Response> {
//!     println!("Received ping event.");
//!     ctx.emit("pong", ()).await?;
//!
//!     Ok(Response::empty())
//! }
//!
//! pub struct MyNamespace;
//!
//! impl MyNamespace {
//!     async fn ping(_ctx: &Context, _event: Event) -> IPCResult<Response> {
//!         println!("My namespace received a ping");
//!         Ok(Response::empty())
//!     }
//! }
//!
//! impl NamespaceProvider for MyNamespace {
//!     fn name() -> &'static str {"my_namespace"}
//!
//!     fn register(handler: &mut EventHandler) {
//!         events!(handler,
//!             "ping" => Self::ping,
//!             "ping2" => Self::ping
//!         );
//!     }
//!}
//!
//! #[tokio::main]
//! async fn main() {
//!     // create the client
//!     use std::net::ToSocketAddrs;
//! let ctx = IPCBuilder::<TcpListener>::new()
//!         .address("127.0.0.1:2020".to_socket_addrs().unwrap().next().unwrap())
//!         // register callback
//!         .on("ping", callback!(handle_ping))
//!         .namespace("mainspace-client")
//!         // register callback inline
//!         .on("something", callback!(ctx, event, async move {
//!             println!("I think the server did something");
//!             ctx.emit_to("mainspace-server", "ok", ()).await?;
//!             Ok(Response::empty())
//!         }))
//!         .build()
//!         .add_namespace(namespace!(MyNamespace))
//!         .build_client().await.unwrap();
//!
//!     // emit an initial event
//!     let response = ctx.emit("ping", ()).await_reply().await.unwrap();
//!     assert_eq!(response.name(), "pong");
//! }
//! ```
//!
//! Server Example:
//! ```no_run
//! use std::net::ToSocketAddrs;
//! use typemap_rev::TypeMapKey;
//! use bromine::IPCBuilder;
//! use bromine::prelude::*;
//! use tokio::net::TcpListener;
//!
//! struct MyKey;
//!
//! impl TypeMapKey for MyKey {
//!     type Value = u32;
//! }
//!
//! // create the server
//!# async fn a() {
//! IPCBuilder::<TcpListener>::new()
//!     .address("127.0.0.1:2020".to_socket_addrs().unwrap().next().unwrap())
//!     // register callback
//!     .on("ping", callback!(ctx, event, async move {
//!         println!("Received ping event.");
//!         ctx.emit("pong", ()).await?;
//!         Ok(Response::empty())
//!     }))
//!     .namespace("mainspace-server")
//!     .on("do-something", callback!(ctx, event, async move {
//!         println!("Doing something");
//!         {
//!             // access data
//!             let mut data = ctx.data.write().await;
//!             let mut my_key = data.get_mut::<MyKey>().unwrap();
//!             *my_key += 1;
//!         }
//!         ctx.emit_to("mainspace-client", "something", ()).await?;
//!         Ok(Response::empty())
//!     }))
//!     .build()
//!     // store additional data
//!     .insert::<MyKey>(3)
//!     .build_server().await.unwrap();
//! # }
//! ```

#[cfg(all(
    feature = "serialize",
    not(any(
        feature = "serialize_bincode",
        feature = "serialize_rmp",
        feature = "serialize_postcard",
        feature = "serialize_json"
    ))
))]
compile_error!("Feature 'serialize' cannot be used by its own. Choose one of 'serialize_rmp', 'serialize_bincode', 'serialize_postcard', 'serialize_json instead.");

pub mod error;
mod events;
pub mod ipc;
mod macros;
mod namespaces;
pub mod protocol;

pub use events::error_event;
pub use events::event;
pub use events::event_handler;
pub use events::payload;
pub use ipc::builder::IPCBuilder;
pub use ipc::context;
pub use macros::*;
pub use namespaces::builder::NamespaceBuilder;
pub use namespaces::namespace;
pub use namespaces::provider_trait;

pub mod prelude {
    pub use crate::error::Error as IPCError;
    pub use crate::error::Result as IPCResult;
    pub use crate::event::Event;
    pub use crate::event_handler::{EventHandler, Response};
    pub use crate::ipc::context::Context;
    pub use crate::ipc::context::{PoolGuard, PooledContext};
    pub use crate::ipc::*;
    pub use crate::macros::*;
    pub use crate::namespace::Namespace;
    pub use crate::namespaces::builder::NamespaceBuilder;
    pub use crate::namespaces::provider_trait::*;
    pub use crate::payload::*;
    pub use crate::protocol::*;
    pub use crate::*;
}
