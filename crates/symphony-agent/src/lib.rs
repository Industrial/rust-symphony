//! Agent runner protocol types (SPEC §10): NDJSON message parsing.
//!
//! See `docs/09-agent-runner.md`.

mod protocol;

pub use protocol::{AgentMessage, parse_line};
