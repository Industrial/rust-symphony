//! Orchestration state machine types (SPEC §7, §8): messages, claim state, scheduling.
//!
//! See `docs/06-orchestration.md`, `docs/07-polling-scheduling.md`.

mod claim_state;
mod messages;
mod scheduling;

pub use claim_state::{ClaimState, claim_state};
pub use messages::{AgentUpdatePayload, OrchestratorMessage, WorkerExitReason};
pub use scheduling::{retry_delay_ms, sort_for_dispatch};
