# 06 — Orchestration State Machine

Rust implementation notes for **SPEC §7**. Uses **Tokio**; **single-owner + message passing**; **plain enums and structs** (no FSM crate). Retries are handled in a **tick-based** check (see [07-polling-scheduling.md](07-polling-scheduling.md)).

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Design Choices

| Choice | Decision | Rationale |
|--------|----------|-----------|
| Runtime | **Tokio** | Async I/O for tracker, subprocess, and timers; one event loop. |
| State ownership | **Single owner + message passing** | One orchestrator task owns `OrchestratorState`; workers and tick loop send messages; no `Arc<RwLock<State>>` or shared mutable state. |
| State machine | **Plain Rust enums/structs** | `RunAttemptStatus`, claim state (Unclaimed/Claimed/Running/RetryQueued/Released) encoded in the state struct and transition logic in match/if; no external FSM crate. |
| Retry timers | **Tick-based check** | On each poll tick (or a short interval), the orchestrator checks `retry_attempts` for entries with `due_at <= now` and processes them (re-dispatch or release). Fits single-owner design and avoids many spawned timer tasks. |

---

## 7.1 Issue Orchestration States (SPEC §7.1)

These are **claim states**, not tracker states. They are implicit in the orchestrator state:

- **Unclaimed**: Issue not in `running` and not in `retry_attempts` (and not in `claimed`).
- **Claimed**: Issue in `claimed` set (either in `running` or in `retry_attempts`).
- **Running**: Issue in `running` map.
- **RetryQueued**: Issue in `retry_attempts` (and in `claimed`).
- **Released**: Claim removed (removed from `claimed` and `retry_attempts`; if was running, also removed from `running`).

No separate enum is required: derive claim state from `state.running.contains(id)`, `state.claimed.contains(id)`, and `state.retry_attempts.contains_key(id)`.

---

## 7.2 Run Attempt Lifecycle (SPEC §7.2)

Use the **RunAttemptStatus** enum from [03-domain-model.md](03-domain-model.md):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAttemptStatus {
    PreparingWorkspace,
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
```

Transitions are driven by the agent runner and orchestrator: the runner reports progress and terminal status; the orchestrator sets status on the running entry or moves the issue to retry/completed.

---

## 7.3 Transition Triggers (SPEC §7.3)

The orchestrator task receives **messages** and applies transitions to its state. Recommended message enum:

```rust
pub enum OrchestratorMessage {
    /// Poll tick: reconcile, validate, fetch candidates, dispatch, process due retries.
    PollTick,
    /// Worker finished (normal or abnormal).
    WorkerExit {
        issue_id: String,
        reason: WorkerExitReason,
        runtime_seconds: f64,
        token_totals: (u64, u64, u64),
    },
    /// Live update from agent (session metadata, tokens, rate limits).
    AgentUpdate {
        issue_id: String,
        update: AgentUpdatePayload,
    },
    /// Request to terminate a running worker (reconciliation or stall).
    TerminateWorker { issue_id: String, cleanup_workspace: bool },
}

pub enum WorkerExitReason {
    Normal,
    Failed(String),
    TimedOut,
    Stalled,
    CanceledByReconciliation,
}
```

**Transition logic (conceptual)**:

- **PollTick**: Run reconciliation (stall check + tracker state refresh), then validation, then fetch candidates, then for each retry entry with `due_at <= now` handle retry (fetch candidates, find issue, dispatch or release), then dispatch new issues until slots full. Notify observability.
- **WorkerExit**: Remove from `running`; add runtime and tokens to `agent_totals`; if Normal, insert into `retry_attempts` with attempt 1 and short delay (1000 ms); else insert with exponential backoff. Remove from `retry_attempts` when adding to running. Keep `claimed` in sync (add when dispatching, remove when releasing).
- **AgentUpdate**: If `issue_id` in `running`, update the running entry’s `LiveSession` and optional `agent_rate_limits`.
- **TerminateWorker**: Signal the worker task to stop (e.g. via a cancel handle or channel); on receipt of WorkerExit with that id, apply WorkerExit logic; if `cleanup_workspace`, workspace manager removes the workspace (or schedule cleanup).

Idempotency: before dispatching, always check `!state.claimed.contains(&issue.id)` and `!state.running.contains_key(&issue.id)` and slot availability.

---

## 7.4 Idempotency and Recovery (SPEC §7.4)

- **Single authority**: Only the orchestrator task mutates `OrchestratorState`; all transitions run in that task’s message handler.
- **Pre-dispatch checks**: Require `!claimed.contains(issue_id)` and `!running.contains_key(issue_id)` and `available_slots > 0` (and per-state slots if configured).
- **Reconciliation before dispatch**: On every PollTick, run reconciliation (stall + tracker state refresh) before fetching candidates and dispatching.
- **Restart**: No persistent orchestrator state; on startup run terminal workspace cleanup, then start the poll loop; re-fetch candidates and dispatch as usual.
- **Retry entry replacement**: When scheduling a retry, replace any existing `retry_attempts` entry for that issue (no separate “cancel timer” needed with tick-based retries; just overwrite or remove and re-insert with new `due_at`).

---

## Orchestrator Task Structure (Tokio)

```text
1. Load config, validate, run startup terminal cleanup.
2. Spawn orchestrator task that:
   - Owns OrchestratorState.
   - Receives on a mpsc (or similar) channel for OrchestratorMessage.
   - Loop: select on (tick interval, channel).
     - On interval: send PollTick to self (or handle tick inline).
     - On PollTick: reconcile, validate, fetch candidates, process due retries, dispatch; notify.
     - On WorkerExit: update state, update totals, schedule retry (write into retry_attempts with due_at = now + delay_ms).
     - On AgentUpdate: update running entry.
     - On TerminateWorker: send cancel to worker; state update happens on subsequent WorkerExit.
3. Spawn worker tasks via tokio::spawn; workers send WorkerExit and AgentUpdate on a channel back to the orchestrator.
4. Poll tick interval: use tokio::time::interval with config.poll_interval_ms; on each tick run the PollTick logic and, inside it, check retry_attempts for due_at <= now and process those entries (then remove them from retry_attempts when dispatching or releasing).
```

Retry “timer”: no separate timer per issue. Each tick, compute `now` (monotonic or wall clock as used when setting `due_at`); for each entry in `retry_attempts` where `due_at <= now`, run the retry handling (fetch candidates, find issue, dispatch or release), and remove the entry. Continuation retry (1 s) and backoff (up to 5 m) are satisfied as long as the tick interval is not larger than the minimum delay (e.g. 1 s); use a tick interval ≤ 1000 ms or a dedicated short interval for retry checks if poll_interval_ms is large.

---

## References

- [SPEC.md](SPEC.md) §7 — Orchestration State Machine  
- [03-domain-model.md](03-domain-model.md) — OrchestratorState, RunAttemptStatus, RetryEntry  
- [07-polling-scheduling.md](07-polling-scheduling.md) — Poll loop, reconciliation, retry backoff, tick-based retry check
