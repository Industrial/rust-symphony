//! Orchestration state machine types (SPEC §7, §8): messages, claim state, scheduling.
//!
//! See `docs/06-orchestration.md`, `docs/07-polling-scheduling.md`.

mod claim_state;
mod messages;
mod scheduling;
mod snapshot;
mod transitions;

pub use claim_state::{ClaimState, claim_state};
pub use messages::{AgentUpdatePayload, OrchestratorMessage, WorkerExitReason};
pub use scheduling::{retry_delay_ms, sort_for_dispatch};
pub use snapshot::{OrchestratorSnapshot, RetryRow, SessionRow, snapshot_from_state};
pub use transitions::{
  apply_agent_update, apply_worker_exit, available_slots, can_dispatch, release_claim,
  remove_retry_on_dispatch,
};
