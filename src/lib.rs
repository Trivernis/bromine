//! This project provides an ipc server and client implementation using
//! messagepack. All calls are asynchronous and event based.
//! Client Example:
//! ```no_run
//! use rmp_ipc::IPCBuilder;
//! // create the client
//! # async fn a() {
//!
//! let ctx = IPCBuilder::new()
//!     .address("127.0.0.1:2020")
//!     // register callback
//!     .on("ping", |ctx, event| Box::pin(async move {
//!         println!("Received ping event.");
//!         ctx.emitter.emit_response(event.id(), "pong", ()).await?;
//!         Ok(())
//!     }))
//!     .build_client().await.unwrap();
//!
//! // emit an initial event
//! let response = ctx.emitter.emit("ping", ()).await.unwrap().await_reply(&ctx).await.unwrap();
//! assert_eq!(response.name(), "pong");
//! # }
//! ```
//!
//! Server Example:
//! ```no_run
//! use rmp_ipc::IPCBuilder;
//! // create the server
//!# async fn a() {
//! IPCBuilder::new()
//!     .address("127.0.0.1:2020")
//!     // register callback
//!     .on("ping", |ctx, event| Box::pin(async move {
//!         println!("Received ping event.");
//!         ctx.emitter.emit_response(event.id(), "pong", ()).await?;
//!         Ok(())
//!     }))
//!     .build_server().await.unwrap();
//! # }
//! ```

#[cfg(test)]
mod tests;

pub mod error;
mod events;
mod ipc;

pub use events::error_event;
pub use events::event::Event;
pub use ipc::builder::IPCBuilder;
pub use ipc::*;
