//! Orchestration state machine types (SPEC §7): messages, claim state.
//!
//! See `docs/06-orchestration.md`.

mod claim_state;
mod messages;

pub use claim_state::{ClaimState, claim_state};
pub use messages::{AgentUpdatePayload, OrchestratorMessage, WorkerExitReason};
