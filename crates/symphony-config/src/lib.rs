//! Config layer (SPEC §6): typed config, $VAR resolution, dispatch validation.
//!
//! See `docs/05-configuration.md`.

mod build;
mod config;
mod error;
mod resolve;

pub use build::from_workflow_config;
pub use config::{
  AgentConfig, HooksConfig, PollingConfig, RunnerConfig, ServiceConfig, TrackerConfig,
  WorkspaceConfig,
};
pub use error::{ConfigError, ConfigValidationError};
pub use resolve::{resolve_var, resolve_workspace_root};
