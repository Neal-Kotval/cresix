//! Stable domain contracts shared by the control plane, runner, and CLI.

pub mod manifest;
pub mod model;
pub mod permissions;

pub use manifest::{ManifestError, ProjectManifest};
pub use model::*;
pub use permissions::{Action, Role};
