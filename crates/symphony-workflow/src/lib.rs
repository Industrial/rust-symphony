//! Workflow spec (SPEC §5): path resolution, YAML front matter, prompt body.
//!
//! See `docs/04-workflow-spec.md`.

mod error;
mod loader;
mod path;

pub use error::WorkflowError;
pub use loader::load_workflow;
pub use path::resolve_workflow_path;

use std::path::PathBuf;

/// Resolve workflow path, read file, and parse into `WorkflowDefinition`.
pub fn load_workflow_file(
  explicit: Option<PathBuf>,
) -> Result<symphony_domain::WorkflowDefinition, WorkflowError> {
  let path = resolve_workflow_path(explicit)?;
  let content = std::fs::read_to_string(&path).map_err(WorkflowError::Io)?;
  load_workflow(&content)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn load_workflow_file_missing_path() {
    let r = load_workflow_file(Some(PathBuf::from("/nonexistent/WORKFLOW.md")));
    assert!(matches!(r, Err(WorkflowError::MissingWorkflowFile(_))));
  }

  #[test]
  fn load_workflow_file_success() {
    let temp = std::env::temp_dir().join("symphony_workflow_test.WORKFLOW.md");
    let content = "---\npoll_interval_ms: 100\n---\nTest prompt.";
    std::fs::File::create(&temp)
      .and_then(|mut f| f.write_all(content.as_bytes()))
      .expect("write temp file");
    let result = load_workflow_file(Some(temp.clone()));
    let _ = std::fs::remove_file(&temp);
    let w = result.expect("load_workflow_file");
    assert_eq!(w.prompt_template, "Test prompt.");
    assert_eq!(w.config["poll_interval_ms"], 100);
  }

  #[test]
  fn load_workflow_file_invalid_content() {
    let temp = std::env::temp_dir().join("symphony_workflow_invalid.WORKFLOW.md");
    std::fs::File::create(&temp)
      .and_then(|mut f| f.write_all(b"---\n  bad: yaml: [[[\n---\nbody"))
      .expect("write temp file");
    let result = load_workflow_file(Some(temp.clone()));
    let _ = std::fs::remove_file(&temp);
    assert!(matches!(result, Err(WorkflowError::WorkflowParseError(_))));
  }
}
