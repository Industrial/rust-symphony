//! Config validation and build errors (SPEC §6.3).

use validator::ValidationErrors;

#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
  #[error("tracker config: {0}")]
  Tracker(ValidationErrors),

  #[error("runner config: {0}")]
  Runner(ValidationErrors),
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

  #[error("deserialize: {0}")]
  Deserialize(String),
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
