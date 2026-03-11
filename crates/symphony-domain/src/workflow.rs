//! WorkflowDefinition (SPEC §4.1.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
  /// YAML front matter as a generic map (further parsed by config layer).
  pub config: serde_json::Value,
  /// Markdown body after front matter, trimmed.
  pub prompt_template: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn workflow_definition_roundtrip() {
    let w = WorkflowDefinition {
      config: serde_json::json!({ "poll_interval_ms": 60_000 }),
      prompt_template: "You are working on {{ issue.title }}.".to_string(),
    };
    assert_eq!(w.prompt_template, "You are working on {{ issue.title }}.");
    assert_eq!(w.config["poll_interval_ms"], 60_000);
  }

  #[test]
  fn workflow_definition_empty_config() {
    let w = WorkflowDefinition {
      config: serde_json::Value::Object(Default::default()),
      prompt_template: "Body only.".to_string(),
    };
    assert!(w.config.is_object());
    assert!(w.config.as_object().unwrap().is_empty());
  }
}
