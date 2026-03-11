# 15 — Reference Algorithms (Language-Agnostic → Rust)

Rust implementation notes for **SPEC §16**. The spec gives pseudocode; this doc maps it to the structures and patterns used in the Rust implementation (orchestrator task, messages, tick loop, worker spawn).

---

## 16.1 Service Startup (SPEC §16.1)

- **configure_logging**: Initialize `tracing_subscriber` ([12-logging-observability.md](12-logging-observability.md)).
- **start_observability_outputs**: Optional HTTP server or status surface; spawn if configured.
- **start_workflow_watch**: File watcher (notify or poll) + reload callback that re-reads workflow and applies config ([04-workflow-spec.md](04-workflow-spec.md), [05-configuration.md](05-configuration.md)).
- **state**: Build `OrchestratorState` with defaults from config ([03-domain-model.md](03-domain-model.md)).
- **validate_dispatch_config**: Config layer validation; fail startup on error.
- **startup_terminal_workspace_cleanup**: Tracker `fetch_issues_by_states(terminal_states)`; for each issue, workspace manager remove dir ([07-polling-scheduling.md](07-polling-scheduling.md), [08-workspace-management.md](08-workspace-management.md)).
- **schedule_tick(0)**: Start the poll loop (immediate first tick); then `interval.tick()` every `poll_interval_ms`.
- **event_loop**: Orchestrator task that receives `OrchestratorMessage` and runs the tick and message handlers ([06-orchestration.md](06-orchestration.md)).

---

## 16.2 Poll-and-Dispatch Tick (SPEC §16.2)

- **on_tick**: Send `PollTick` to orchestrator (or handle inline in the interval branch).
- **reconcile_running_issues**: Stall check + tracker state refresh ([07-polling-scheduling.md](07-polling-scheduling.md)).
- **validate_dispatch_config**: Re-validate; on failure skip dispatch, schedule next tick, return.
- **tracker.fetch_candidate_issues()**: GitHub client; on failure log, schedule next tick, return.
- **sort_for_dispatch**: Priority, created_at, identifier ([07-polling-scheduling.md](07-polling-scheduling.md)).
- **for issue in sort_for_dispatch(issues)**: Check slots and eligibility; if eligible, `dispatch_issue` (spawn worker, update state); break when no slots.
- **notify_observers**: Snapshot or log; optional HTTP push.
- **schedule_tick(poll_interval_ms)**: Next `interval.tick()`.

---

## 16.3 Reconcile Active Runs (SPEC §16.3)

- **reconcile_stalled_runs**: For each running entry, if elapsed > `runner.stall_timeout_ms`, send TerminateWorker.
- **tracker.fetch_issue_states_by_ids(running_ids)**: GitHub client; on failure return state unchanged.
- **for issue in refreshed**: Terminal → terminate + cleanup_workspace; active → update running entry snapshot; else → terminate without cleanup.

---

## 16.4 Dispatch One Issue (SPEC §16.4)

- **spawn_worker**: `tokio::spawn(run_agent_attempt(...))`; pass a channel to send `WorkerExit` and `AgentUpdate` back to the orchestrator.
- **running entry**: Insert into `state.running` with LiveSession defaults, `started_at`, etc.; add `issue_id` to `claimed`; remove from `retry_attempts`.
- **on spawn failure**: Schedule retry with error.

---

## 16.5 Worker Attempt (SPEC §16.5)

- **workspace_manager.create_for_issue**: [08-workspace-management.md](08-workspace-management.md).
- **run_hook("before_run")**: Run hook in a task with timeout; on failure return error.
- **app_server.start_session**: [09-agent-runner.md](09-agent-runner.md) handshake.
- **build_turn_prompt**: [11-prompt-construction.md](11-prompt-construction.md).
- **app_server.run_turn**: Send turn/start; read lines until turn/completed or turn/failed/timeout; forward AgentUpdate on channel.
- **refreshed_issue**: `tracker.fetch_issue_states_by_ids([issue.id])`; update local issue; if not active break.
- **max_turns**: Break when `turn_number >= max_turns`.
- **app_server.stop_session**: Kill subprocess; run after_run hook (best-effort).

---

## 16.6 Worker Exit and Retry (SPEC §16.6)

- **on_worker_exit**: Remove from running; add runtime/tokens to agent_totals; if normal, schedule retry with attempt 1 and 1000 ms delay; else exponential backoff ([07-polling-scheduling.md](07-polling-scheduling.md)).
- **on_retry_timer** (tick-based): For entries with `due_at <= now`, fetch candidates, find issue, dispatch or release or requeue ([07-polling-scheduling.md](07-polling-scheduling.md)).

---

## References

- [SPEC.md](SPEC.md) §16 — Reference Algorithms  
- [06-orchestration.md](06-orchestration.md), [07-polling-scheduling.md](07-polling-scheduling.md), [08-workspace-management.md](08-workspace-management.md), [09-agent-runner.md](09-agent-runner.md)
