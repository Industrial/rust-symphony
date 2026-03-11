//! Load WORKFLOW.md content: split front matter and body (SPEC §5.2).
//!
//! Option B: regex split + serde_yaml. No leading `---` → entire file is prompt body; config = empty map.

use once_cell::sync::Lazy;
use regex::Regex;
use serde_yaml::Value;

use symphony_domain::WorkflowDefinition;

use crate::WorkflowError;

static FRONT_MATTER_RE: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---\r?\n(.*)").expect("front matter regex"));

/// Split content into optional YAML block and body.
/// If the file does not start with `---`, returns `(None, content.trim())`.
fn split_front_matter(content: &str) -> (Option<&str>, &str) {
  if let Some(caps) = FRONT_MATTER_RE.captures(content) {
    let yaml = caps.get(1).map(|m| m.as_str());
    let body = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
    (yaml, body)
  } else {
    (None, content.trim())
  }
}

/// Load workflow from file content.
///
/// - No leading `---` → entire file is prompt body; config = empty map.
/// - YAML must decode to a map; non-map → `WorkflowFrontMatterNotAMap`.
pub fn load_workflow(content: &str) -> Result<WorkflowDefinition, WorkflowError> {
  let (yaml_opt, body) = split_front_matter(content);
  let config = match yaml_opt {
    None => serde_json::Value::Object(Default::default()),
    Some(yaml) => {
      let v: Value =
        serde_yaml::from_str(yaml).map_err(|e| WorkflowError::WorkflowParseError(e.to_string()))?;
      if !v.is_mapping() {
        return Err(WorkflowError::WorkflowFrontMatterNotAMap);
      }
      serde_json::to_value(v).map_err(|e| WorkflowError::WorkflowParseError(e.to_string()))?
    }
  };
  Ok(WorkflowDefinition {
    config,
    prompt_template: body.to_string(),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn load_workflow_body_only() {
    let content = "You are working on {{ issue.title }}.";
    let w = load_workflow(content).unwrap();
    assert_eq!(w.prompt_template, "You are working on {{ issue.title }}.");
    assert!(w.config.is_object());
    assert!(w.config.as_object().unwrap().is_empty());
  }

  #[test]
  fn load_workflow_with_front_matter() {
    let content = r#"---
poll_interval_ms: 60000
tracker:
  repo: owner/repo
---
You are working on {{ issue.title }}."#;
    let w = load_workflow(content).unwrap();
    assert_eq!(w.prompt_template, "You are working on {{ issue.title }}.");
    assert_eq!(w.config["poll_interval_ms"], 60000);
    assert_eq!(w.config["tracker"]["repo"], "owner/repo");
  }

  #[test]
  fn load_workflow_front_matter_not_a_map() {
    let content = "---\n- a\n- b\n---\nbody";
    let r = load_workflow(content);
    assert!(matches!(r, Err(WorkflowError::WorkflowFrontMatterNotAMap)));
  }

  #[test]
  fn load_workflow_invalid_yaml() {
    let content = "---\n  invalid: yaml: [[[\n---\nbody";
    let r = load_workflow(content);
    assert!(matches!(r, Err(WorkflowError::WorkflowParseError(_))));
  }

  #[test]
  fn load_workflow_empty_body_after_front_matter() {
    let content = "---\nkey: value\n---\n";
    let w = load_workflow(content).unwrap();
    assert_eq!(w.prompt_template, "");
    assert_eq!(w.config["key"], "value");
  }

  #[test]
  fn load_workflow_body_only_trimmed() {
    let content = "  \n\n  prompt line  \n  ";
    let w = load_workflow(content).unwrap();
    assert_eq!(w.prompt_template, "prompt line");
    assert!(w.config.as_object().unwrap().is_empty());
  }

  #[test]
  fn load_workflow_front_matter_with_crlf() {
    let content = "---\r\npoll_interval_ms: 30\r\n---\r\nbody";
    let w = load_workflow(content).unwrap();
    assert_eq!(w.prompt_template, "body");
    assert_eq!(w.config["poll_interval_ms"], 30);
  }
}
