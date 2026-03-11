//! Config layer (SPEC §6): typed config, $VAR resolution, dispatch validation.
//!
//! See `docs/05-configuration.md`.

mod build;
mod config;
mod error;
mod resolve;

pub use config::{RunnerConfig, ServiceConfig, TrackerConfig};
pub use error::{ConfigError, ConfigValidationError};
pub use build::from_workflow_config;
pub use resolve::resolve_var;
