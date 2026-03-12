//! Config layer (SPEC §6): typed config, $VAR resolution, dispatch validation.
//!
//! See `docs/05-configuration.md`.

#![allow(clippy::missing_docs_in_private_items)]

mod build;
mod config;
mod error;
mod resolve;

pub use build::from_workflow_config;
pub use config::{
  AgentConfig, HooksConfig, PollingConfig, RunnerConfig, RunnerType, ServiceConfig, TrackerConfig,
  WorktreeConfig,
};
pub use error::{ConfigError, ConfigValidationError};
pub use resolve::{resolve_var, resolve_worktree_root};
