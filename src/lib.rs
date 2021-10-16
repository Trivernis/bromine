//! This project provides an ipc server and client implementation using
//! messagepack. All calls are asynchronous and event based.
//! Client Example:
//! ```no_run
//! use rmp_ipc::{callback, Event, context::Context, IPCBuilder, error::Result};
//!
//! /// Callback ping function
//! async fn handle_ping(ctx: &Context, event: Event) -> Result<()> {
//!     println!("Received ping event.");
//!     ctx.emitter.emit_response(event.id(), "pong", ()).await?;
//!
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     // create the client
//!     let ctx = IPCBuilder::new()
//!         .address("127.0.0.1:2020")
//!         // register callback
//!         .on("ping", callback!(handle_ping))
//!         .namespace("mainspace-client")
//!         // register callback inline
//!         .on("something", callback!(ctx, event, async move {
//!             println!("I think the server did something");
//!             ctx.emitter.emit_response_to(event.id(), "mainspace-server", "ok", ()).await?;
//!             Ok(())
//!         }))
//!         .build()
//!         .build_client().await.unwrap();
//!
//!     // emit an initial event
//!     let response = ctx.emitter.emit("ping", ()).await.unwrap().await_reply(&ctx).await.unwrap();
//!     assert_eq!(response.name(), "pong");
//! }
//! ```
//!
//! Server Example:
//! ```no_run
//! use typemap_rev::TypeMapKey;
//! use rmp_ipc::IPCBuilder;
//! use rmp_ipc::callback;
//!
//! struct MyKey;
//!
//! impl TypeMapKey for MyKey {
//!     type Value = u32;
//! }
//!
//! // create the server
//!# async fn a() {
//! IPCBuilder::new()
//!     .address("127.0.0.1:2020")
//!     // register callback
//!     .on("ping", callback!(ctx, event, async move {
//!         println!("Received ping event.");
//!         ctx.emitter.emit_response(event.id(), "pong", ()).await?;
//!         Ok(())
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
//!         ctx.emitter.emit_response_to(event.id(), "mainspace-client", "something", ()).await?;
//!         Ok(())
//!     }))
//!     .build()
//!     // store additional data
//!     .insert::<MyKey>(3)
//!     .build_server().await.unwrap();
//! # }
//! ```

#[cfg(test)]
mod tests;

pub mod error;
mod events;
mod ipc;
mod macros;
mod namespaces;

pub use events::error_event;
pub use events::event::Event;
pub use ipc::builder::IPCBuilder;
pub use ipc::*;
pub use macros::*;
pub use namespaces::builder::NamespaceBuilder;
pub use namespaces::namespace::Namespace;
