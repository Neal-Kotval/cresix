//! Stable contracts shared by Cresix Cloud, its connector, and clients.
//!
//! This crate deliberately contains no persistence, HTTP, or credential
//! verification. It validates presentation identifiers and protocol shape;
//! each service must still authenticate and authorize every operation.

mod api;
mod identifiers;
mod model;
mod relay;
mod token;

pub use api::*;
pub use identifiers::*;
pub use model::*;
pub use relay::*;
pub use token::*;
