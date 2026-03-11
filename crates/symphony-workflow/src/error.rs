//! Workflow error types (SPEC §5.5).

use std::path::PathBuf;

/// Errors from workflow path resolution and loading.
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
  #[error("missing workflow file: {0}")]
  MissingWorkflowFile(PathBuf),

  #[error("io error: {0}")]
  Io(#[from] std::io::Error),

  #[error("workflow parse error: {0}")]
  WorkflowParseError(String),

  #[error("workflow front matter is not a map")]
  WorkflowFrontMatterNotAMap,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_workflow_file_display() {
    let e = WorkflowError::MissingWorkflowFile(PathBuf::from("/tmp/foo.md"));
    let s = e.to_string();
    assert!(s.contains("missing workflow file"));
    assert!(s.contains("foo.md"));
  }

  #[test]
  fn workflow_front_matter_not_a_map_display() {
    let e = WorkflowError::WorkflowFrontMatterNotAMap;
    assert!(e.to_string().contains("not a map"));
  }

  #[test]
  fn io_error_from_std_io_error() {
    let err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let workflow_err: WorkflowError = err.into();
    assert!(matches!(workflow_err, WorkflowError::Io(_)));
  }
}
