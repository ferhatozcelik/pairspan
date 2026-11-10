//! Pairspan PC side. The desktop app runs an [`agent::Agent`] and serves [`rpc`] on a local
//! socket; the `pairspan` CLI and MCP server talk to that socket. See docs/protocol.md.

pub mod adb;
pub mod agent;
pub mod automation;
pub mod crypto;
pub mod identity;
pub mod rpc;
pub mod store;
pub mod wire;

pub use agent::{Agent, Event, Options, Status};
