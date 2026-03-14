//! Config validation and build errors (SPEC §6.3).

use validator::ValidationErrors;

#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
  #[error("tracker config: {0}")]
  Tracker(ValidationErrors),

  #[error("runner config: {0}")]
  Runner(ValidationErrors),

  #[error("worktree config: {0}")]
  Worktree(String),
}

impl From<ValidationErrors> for ConfigValidationError {
  fn from(e: ValidationErrors) -> Self {
    ConfigValidationError::Tracker(e)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
  #[error("missing or invalid config key: {0}")]
  MissingKey(String),

  #[error("validation: {0}")]
  Validation(#[from] ConfigValidationError),

  #[error("invalid config: {0}")]
  InvalidConfig(String),

  #[error("deserialize: {0}")]
  Deserialize(String),

  #[error("path resolution: {0}")]
  Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::TrackerConfig;
  use validator::Validate;

  #[test]
  fn config_validation_error_from_validate() {
    let t = TrackerConfig {
      repo: "".to_string(),
      api_key: "k".to_string(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: "claimed".into(),
      pr_open_label: "pr-open".into(),
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: "main".into(),
    };
    let errs = t.validate().unwrap_err();
    let e = ConfigValidationError::Tracker(errs);
    assert!(e.to_string().contains("tracker"));
  }

  #[test]
  fn config_error_missing_key_display() {
    let e = ConfigError::MissingKey("tracker".into());
    assert!(e.to_string().contains("tracker"));
  }
}
