//! File discovery and path resolution (SPEC §5.1).

use std::path::PathBuf;

use crate::WorkflowError;

/// Resolve the path to the workflow file.
///
/// Precedence: (1) explicit path from CLI/config, (2) `WORKFLOW.md` in current working directory.
pub fn resolve_workflow_path(explicit: Option<PathBuf>) -> Result<PathBuf, WorkflowError> {
  tracing::trace!("resolve_workflow_path");
  let path = match explicit {
    Some(p) => p,
    None => std::env::current_dir()
      .map_err(WorkflowError::Io)?
      .join("WORKFLOW.md"),
  };
  if path.exists() && path.is_file() {
    Ok(path)
  } else {
    Err(WorkflowError::MissingWorkflowFile(path))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_workflow_path_explicit_missing() {
    let r = resolve_workflow_path(Some(PathBuf::from("/nonexistent/WORKFLOW.md")));
    assert!(matches!(r, Err(WorkflowError::MissingWorkflowFile(_))));
  }

  #[test]
  fn resolve_workflow_path_explicit_exists() {
    let temp = std::env::current_dir().unwrap().join("Cargo.toml");
    if temp.exists() {
      let r = resolve_workflow_path(Some(temp));
      assert!(r.is_ok());
    }
  }

  #[test]
  fn resolve_workflow_path_none_uses_cwd_workflow_md() {
    let r = resolve_workflow_path(None);
    match &r {
      Ok(p) => {
        assert_eq!(p.file_name().unwrap(), "WORKFLOW.md");
        assert!(
          p.parent()
            .unwrap()
            .ends_with(std::env::current_dir().unwrap())
        );
      }
      Err(WorkflowError::MissingWorkflowFile(p)) => {
        assert_eq!(p.file_name().unwrap(), "WORKFLOW.md");
      }
      _ => panic!("unexpected error variant"),
    }
  }
}
