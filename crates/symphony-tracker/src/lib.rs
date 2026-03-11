//! Issue tracker integration (SPEC §11): errors and GitHub normalization.
//!
//! See `docs/10-github-tracker.md`.

mod error;
mod normalize;

pub use error::TrackerError;
pub use normalize::github_issue_to_domain;
