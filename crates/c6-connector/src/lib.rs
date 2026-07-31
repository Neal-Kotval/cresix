//! Outbound-only connector for an optional Cresix Cloud relay.
//!
//! The connector is intentionally not a general proxy: configuration accepts
//! one literal loopback HTTP authority and relay requests cannot override it.

pub mod catalog;
pub mod config;
pub mod protocol;
pub mod proxy;
pub mod runtime;

pub use config::{ConnectorConfig, Credentials, LoadedConfig};
