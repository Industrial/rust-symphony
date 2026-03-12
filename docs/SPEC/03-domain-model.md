# 03 — Core Domain Model

Rust implementation notes for **SPEC §4**. Uses **Serde** (with **serde_json**), **chrono**, and **validator**.

**Deliverable:** Unit tests must be written for all code (e.g. each module/file); implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Crates

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
validator = { version = "0.18", features = ["derive"] }
```

- **serde / serde_json**: Serialization for config, API, and any JSON payloads (e.g. Codex protocol).
- **chrono**: `DateTime<Utc>` for timestamps; `Option<DateTime<Utc>>` for nullable; serde feature for (de)serialization.
- **validator**: `#[validate(...)]` and `Validate` trait for required fields, length, and custom rules (e.g. state in allowed set).

---

## 4.1 Entities

### 4.1.1 Issue (SPEC §4.1.1)

Normalized issue from the tracker; used for orchestration, prompt rendering, and observability.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerRef {
    pub id: Option<String>,
    pub identifier: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Issue {
    #[validate(length(min = 1))]
    pub id: String,
    #[validate(length(min = 1))]
    pub identifier: String,
    #[validate(length(min = 1))]
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    #[validate(length(min = 1))]
    pub state: String,
    pub branch_name: Option<String>,
    pub url: Option<String>,
    pub labels: Vec<String>,
    pub blocked_by: Vec<BlockerRef>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

- **Normalization**: `labels` and `state` are stored already normalized (e.g. lowercase) by the tracker client; validator can enforce non-empty `id`, `identifier`, `title`, `state` where needed.
- **Comparison**: For dispatch/reconciliation, compare `state` using a normalized form (e.g. `state.to_lowercase()`); see SPEC §4.2.

---

### 4.1.2 Workflow Definition (SPEC §4.1.2)

Parsed `WORKFLOW.md`: front matter as a map + prompt body. The config part is deserialized into typed structs elsewhere; here we only need the raw shape for loading.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// YAML front matter as a generic map (further parsed by config layer).
    pub config: serde_json::Value,
    /// Markdown body after front matter, trimmed.
    pub prompt_template: String,
}
```

- `config`: Use `serde_json::Value` (or a `HashMap<String, serde_json::Value>`) so the workflow loader stays agnostic; the config layer then deserializes into typed structs (see [05-configuration.md](05-configuration.md)).

---

### 4.1.3 Service Config (typed view)

Typed runtime values live in the config layer (see [05-configuration.md](05-configuration.md)). Domain types that reference them:

- **Poll interval**, **worktree root**, **active/terminal states**, **concurrency limits**, **codex command/timeouts**, **hooks**: all come from the parsed workflow config + env resolution.
- Use **validator** in the config layer to enforce “required after resolution” (e.g. `tracker.repo`, `tracker.api_key`, `codex.command`).

---

### 4.1.4 Worktree (SPEC §4.1.4)

Logical git worktree for one issue; path and key are derived by the worktree manager.

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub worktree_key: String,
    pub created_now: bool,
}
```

- **worktree_key**: Sanitized issue identifier per SPEC §4.2: replace any char not in `[A-Za-z0-9._-]` with `_`. Implement as a free function and use it when creating `Worktree` and when computing paths.

---

### 4.1.5 Run Attempt (SPEC §4.1.5)

One execution attempt for one issue; status is an enum for the lifecycle phases in SPEC §7.2.

```rust
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAttemptStatus {
    PreparingWorktree,
    BuildingPrompt,
    LaunchingAgentProcess,
    InitializingSession,
    StreamingTurn,
    Finishing,
    Succeeded,
    Failed,
    TimedOut,
    Stalled,
    CanceledByReconciliation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAttempt {
    pub issue_id: String,
    pub issue_identifier: String,
    pub attempt: Option<u32>,
    pub worktree_path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub status: RunAttemptStatus,
    pub error: Option<String>,
}
```

- **attempt**: `None` for first run, `Some(n)` for retries/continuation (`n >= 1`).

---

### 4.1.6 Live Session (SPEC §4.1.6)

State for a single running coding-agent subprocess.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LiveSession {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub agent_pid: Option<String>,
    pub last_agent_event: Option<String>,
    pub last_agent_timestamp: Option<DateTime<Utc>>,
    pub last_agent_message: Option<String>,
    pub agent_input_tokens: u64,
    pub agent_output_tokens: u64,
    pub agent_total_tokens: u64,
    pub last_reported_input_tokens: u64,
    pub last_reported_output_tokens: u64,
    pub last_reported_total_tokens: u64,
    pub turn_count: u32,
}
```

- **session_id**: Set from `format!("{}-{}", thread_id, turn_id)` when both are known (SPEC §4.2).

---

### 4.1.7 Retry Entry (SPEC §4.1.7)

Scheduled retry for an issue. `due_at_ms` is monotonic (e.g. `std::time::Instant::elapsed()` or a monotonic timestamp); timer handle is runtime-specific (e.g. `tokio::time::Instant` or a task reference).

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEntry {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_at_ms: u64,
    pub error: Option<String>,
}
```

- Omit `timer_handle` from serialization (or use a type that doesn’t implement Serialize) if this struct is ever persisted or sent over API; in-memory orchestrator can hold a separate map of timers.

---

### 4.1.8 Orchestrator Runtime State (SPEC §4.1.8)

In-memory state owned by the orchestrator. Maps and sets keyed by `issue_id` (String).

```rust
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::{LiveSession, RetryEntry}; // or equivalent path

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningEntry {
    pub identifier: String,
    pub issue: Issue,
    pub session: LiveSession,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub retry_attempt: u32,
    // worker_handle, monitor_handle: runtime-specific, not serialized
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OrchestratorState {
    pub poll_interval_ms: u64,
    pub max_concurrent_agents: u32,
    pub running: HashMap<String, RunningEntry>,
    pub claimed: HashSet<String>,
    pub retry_attempts: HashMap<String, RetryEntry>,
    pub completed: HashSet<String>,
    pub agent_totals: AgentTotals,
    pub agent_rate_limits: Option<serde_json::Value>,
}
```

- **RunningEntry**: Include `Issue` and `LiveSession` so reconciliation can update the issue snapshot and the status surface can show current state.
- **claimed**: Set of issue IDs that are either running or in the retry queue.

---

## 4.2 Stable Identifiers and Normalization (SPEC §4.2)

### Issue ID and identifier

- **id**: Stable tracker ID (e.g. GitHub `node_id` or numeric id as string). Use as map key and for API lookups.
- **identifier**: Human-readable (e.g. `owner/repo#42`). Use in logs and git worktree naming.

### Worktree key

Derive from `issue.identifier`: replace any character not in `[A-Za-z0-9._-]` with `_`.

```rust
pub fn sanitize_worktree_key(identifier: &str) -> String {
    identifier
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
        .collect()
}
```

### Normalized issue state

Compare states after lowercase (e.g. `state.to_lowercase()` when checking `active_states` / `terminal_states`).

### Session ID

`session_id = format!("{}-{}", thread_id, turn_id)` when both are available from the Codex protocol.

---

## Validation (validator crate)

- Use **Validate** on `Issue` (and optionally `BlockerRef`) where you need to guarantee non-empty IDs/title/state before dispatch.
- For config structs (see [05-configuration.md](05-configuration.md)), use **validator** to enforce required fields after `$VAR` resolution (e.g. `tracker.repo`, `tracker.api_key`).
- Custom validators (e.g. “state in allowed list”) can be implemented with `#[validate(custom(function = "my_validate"))]`.

---

## References

- [SPEC.md](SPEC.md) §4 — Core Domain Model
