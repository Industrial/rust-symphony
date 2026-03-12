//! Core domain types for Symphony (SPEC §4).
//!
//! See `docs/03-domain-model.md` for the specification.

pub mod issue;
pub mod orchestrator;
pub mod path_serde;
pub mod retry;
pub mod run_attempt;
pub mod session;
pub mod workflow;
pub mod worktree;

pub use issue::{BlockerRef, Issue};
pub use orchestrator::{AgentTotals, OrchestratorState, RunningEntry};
pub use retry::RetryEntry;
pub use run_attempt::{RunAttempt, RunAttemptStatus};
pub use session::LiveSession;
pub use workflow::WorkflowDefinition;
pub use worktree::{Worktree, sanitize_worktree_key};
