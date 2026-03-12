//! Command-line interface for the symphony binary (argv parsing).
//!
//! This is the only user-facing CLI in the project: how operators invoke the symphony orchestrator.
//! The symphony-agent crate has a "Cli" runner protocol (stream_cli) for talking to the agent
//! process—that is not argv parsing.

use std::path::PathBuf;

use clap::Parser;

/// Symphony orchestrator: poll tracker, dispatch agents per WORKFLOW.md.
#[derive(Debug, Clone, Parser)]
#[command(name = "symphony")]
#[command(
  about = "Run the Symphony orchestrator: poll the tracker, dispatch agents per workflow config."
)]
#[command(long_about = None)]
pub struct Cli {
  /// Run one poll cycle only: load config and workflow, fetch candidates, apply sort and
  /// concurrency rules, log what would be dispatched, then exit. No workers, no git worktrees, no tracker writes.
  #[arg(long)]
  pub dry_run: bool,

  /// Path to WORKFLOW.md (optional; default from env or repo root).
  #[arg(value_name = "WORKFLOW_PATH")]
  pub workflow_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn cli_default() {
    let cli = Cli::parse_from(["symphony"]);
    assert!(!cli.dry_run);
    assert!(cli.workflow_path.is_none());
  }

  #[test]
  fn cli_dry_run() {
    let cli = Cli::parse_from(["symphony", "--dry-run"]);
    assert!(cli.dry_run);
    assert!(cli.workflow_path.is_none());
  }

  #[test]
  fn cli_workflow_path() {
    let cli = Cli::parse_from(["symphony", "/path/to/WORKFLOW.md"]);
    assert!(!cli.dry_run);
    assert_eq!(
      cli.workflow_path.as_deref().map(|p| p.to_path_buf()),
      Some(PathBuf::from("/path/to/WORKFLOW.md"))
    );
  }

  #[test]
  fn cli_dry_run_and_path() {
    let cli = Cli::parse_from(["symphony", "--dry-run", "./WORKFLOW.md"]);
    assert!(cli.dry_run);
    assert_eq!(
      cli.workflow_path.as_deref().map(|p| p.to_path_buf()),
      Some(PathBuf::from("./WORKFLOW.md"))
    );
  }
}
