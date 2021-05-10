//! This project provides an ipc server and client implementation using
//! messagepack. All calls are asynchronous and event based.
//! Client Example:
//! ```rust
//! use rmp_ipc::IPCBuilder;
//! // create the client
//! let emitter = IPCBuilder::new()
//!     .address("127.0.0.1:2020")
//!     // register callback
//!     .on("ping", |_ctx, _event| Box::pin(async move {
//!         println!("Received ping event.");
//!         Ok(())
//!     }))
//!     .build_client().await.unwrap();
//!
//! // emit an initial event
//! emitter.emit("ping", ()).await?;
//! ```
//!
//! Server Example:
//! ```rust
//! use rmp_ipc::IPCBuilder;
//! // create the server
//! IPCBuilder::new()
//!     .address("127.0.0.1:2020")
//!     // register callback
//!     .on("ping", |_ctx, _event| Box::pin(async move {
//!         println!("Received ping event.");
//!         Ok(())
//!     }))
//!     .build_server().await.unwrap();
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
