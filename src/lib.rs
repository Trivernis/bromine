//! This project provides an ipc server and client implementation using
//! messagepack. All calls are asynchronous and event based.

#[cfg(test)]
mod tests;

pub mod error;
mod events;
mod ipc;

pub use events::event::Event;
pub use ipc::builder::IPCBuilder;
pub use ipc::*;
