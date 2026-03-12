//! Issue tracker integration (SPEC §11): errors, GitHub normalization, and API client.
//!
//! See `docs/10-github-tracker.md`.

mod error;
mod filter;
mod normalize;

#[cfg(feature = "client")]
mod client;

pub use error::TrackerError;
pub use filter::{apply_label_filters, issue_passes_label_filters};
pub use normalize::github_issue_to_domain;

#[cfg(feature = "client")]
pub use client::{
  CheckRunInfo, CombinedStatusInfo, ResolvedPr, check_run_conclusion_is_failed,
  commit_status_state_is_failed, fetch_candidate_issues, fetch_check_runs_for_ref,
  fetch_commit_status_for_ref, fetch_has_qualifying_mention, fetch_issue_states_by_ids,
  fetch_issues_by_states, fetch_issues_with_label, parse_issue_number, parse_repo,
  resolve_pr_for_issue,
};
