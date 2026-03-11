# 07 — Polling, Scheduling, and Reconciliation

Rust implementation notes for **SPEC §8**. Implements the poll tick sequence, candidate selection, concurrency, retry backoff, reconciliation, and startup cleanup. Assumes the **single-owner orchestrator** from [06-orchestration.md](06-orchestration.md) and **tick-based retry** handling.

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## 8.1 Poll Loop (SPEC §8.1)

**Startup**: Validate config → startup terminal workspace cleanup (SPEC §8.6) → schedule first tick (delay 0 or immediate) → enter loop.

**Loop**: Use `tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms))`. On each tick (or on a dedicated channel message triggered by the tick):

1. **Reconcile running issues** (SPEC §8.5): stall detection, then tracker state refresh.
2. **Dispatch preflight validation** (SPEC §6.3): workflow load, `tracker.repo`, `tracker.api_key`, `runner.command`. On failure: skip dispatch this tick, log, notify; continue to step 6.
3. **Fetch candidate issues** from tracker (active states for configured repo). On failure: log, skip dispatch, continue to step 6.
4. **Sort** candidates per §8.2 (priority, created_at, identifier).
5. **Process due retries** (tick-based): For each entry in `retry_attempts` with `due_at <= now` (use monotonic or wall clock consistently): fetch candidates, find issue by id; if not found release claim; if found and eligible and slots available dispatch; if found and eligible but no slots requeue with error; if found but not active release claim. Remove processed entries from `retry_attempts`.
6. **Dispatch** new issues: Iterate sorted candidates; for each, if eligible (see §8.2) and slots available, dispatch (spawn worker, insert into `running` and `claimed`, remove from `retry_attempts` if present); stop when no slots remain.
7. **Notify** observability (e.g. send snapshot to status surface or log summary).

**Effective interval**: When workflow config is reloaded, update `poll_interval_ms` and recreate or adjust the interval so the next tick uses the new value.

---

## 8.2 Candidate Selection and Sort (SPEC §8.2)

**Eligibility** (all must hold):

- `issue.id`, `issue.identifier`, `issue.title`, `issue.state` non-empty (or present).
- `issue.state.to_lowercase()` in `active_states` and not in `terminal_states`.
- `!state.running.contains_key(&issue.id)` and `!state.claimed.contains(&issue.id)`.
- Global slots: `running.len() < max_concurrent_agents`.
- Per-state slots: If `max_concurrent_agents_by_state` contains key `state.to_lowercase()`, count running issues with that state; require count < limit.
- Blocker rule: If issue state is the primary active state (e.g. `open`), do not dispatch if any `blocked_by` entry has non-terminal state.

**Sort order** (stable):

1. `priority` ascending (1 = highest; null/unknown last).
2. `created_at` oldest first (None last).
3. `identifier` lexicographic tie-breaker.

```rust
fn sort_for_dispatch(issues: &mut [Issue]) {
    issues.sort_by(|a, b| {
        let p = (a.priority.unwrap_or(i32::MAX)).cmp(&b.priority.unwrap_or(i32::MAX));
        if p != std::cmp::Ordering::Equal { return p; }
        let t = a.created_at.cmp(&b.created_at);
        if t != std::cmp::Ordering::Equal { return t; }
        a.identifier.cmp(&b.identifier)
    });
}
```

---

## 8.3 Concurrency (SPEC §8.3)

- **Global**: `available_slots = max(max_concurrent_agents.saturating_sub(running.len()), 0)`.
- **Per-state**: For state `s` (normalized lowercase), if `max_concurrent_agents_by_state` contains `s`, then `available_for_state = max(limit - count_running_with_state(s), 0)`. When dispatching an issue with state `s`, require both global and per-state availability; decrement conceptually when adding to `running`.

---

## 8.4 Retry and Backoff (SPEC §8.4)

**Creation**: When scheduling a retry, remove any existing `retry_attempts[issue_id]` and insert a new entry with:

- `issue_id`, `identifier`, `error` (optional), `attempt`.
- `due_at_ms`: monotonic timestamp (e.g. `Instant::now() + delay`) or wall clock in ms. Store in a form comparable to “now” (e.g. `Instant::now()` plus `Duration::from_millis(delay)` and store `Instant` for comparison, or store `due_at_ms` as u64 and compare with current monotonic ms).

**Delay formula**:

- **Continuation** (normal worker exit): fixed `1000` ms.
- **Failure-driven**: `delay_ms = min(10_000 * 2^(attempt - 1), agent.max_retry_backoff_ms)`. Cap at configured max (e.g. 300_000 ms).

**Tick-based retry handling** (each tick, after reconciliation):

- `now = monotonic_now_ms()` (or use `Instant::now()` and compare with stored `Instant`).
- For each `(issue_id, entry)` in `retry_attempts` where `entry.due_at <= now` (or `entry.due_instant.elapsed() >= 0`):
  - Fetch candidate issues (same as dispatch).
  - Find issue by `issue_id`. If not found: remove from `claimed` and `retry_attempts`; continue.
  - If found and not candidate-eligible (e.g. no longer active): remove from `claimed` and `retry_attempts`; continue.
  - If found and eligible and slots available: remove from `retry_attempts`, dispatch (add to running, claimed).
  - If found and eligible but no slots: re-insert retry with same attempt (or attempt+1) and new `due_at` (e.g. 1 s later) and `error = "no available orchestrator slots"`.

---

## 8.5 Active Run Reconciliation (SPEC §8.5)

**Part A — Stall detection**:

- For each `(issue_id, entry)` in `state.running`:
  - `elapsed_ms = now - entry.last_agent_timestamp.unwrap_or(entry.started_at)` (use same clock as `runner.stall_timeout_ms`).
  - If `runner.stall_timeout_ms > 0` and `elapsed_ms > runner.stall_timeout_ms`: send `TerminateWorker { issue_id, cleanup_workspace: false }` (or equivalent); when worker exits, schedule retry. Do not remove from `running` until WorkerExit is received.

**Part B — Tracker state refresh**:

- `ids = state.running.keys().cloned().collect::<Vec<_>>()`.
- `refreshed = tracker.fetch_issue_states_by_ids(ids).await` (or sync). On fetch failure: log, skip Part B this tick.
- For each returned issue: if state in `terminal_states`: send TerminateWorker with `cleanup_workspace: true`. If state in `active_states`: update `state.running[issue.id].issue = issue`. Otherwise: send TerminateWorker with `cleanup_workspace: false`.

---

## 8.6 Startup Terminal Workspace Cleanup (SPEC §8.6)

Before starting the poll loop:

1. Fetch issues in terminal states: `tracker.fetch_issues_by_states(terminal_states)`.
2. For each issue’s `identifier`, compute workspace path and remove the directory (via workspace manager). If the directory does not exist, ignore.
3. If the fetch fails: log warning, continue startup (do not block).

---

## References

- [SPEC.md](SPEC.md) §8 — Polling, Scheduling, and Reconciliation  
- [06-orchestration.md](06-orchestration.md) — Orchestrator messages and state  
- [10-github-tracker.md](10-github-tracker.md) — fetch_candidate_issues, fetch_issue_states_by_ids, fetch_issues_by_states
