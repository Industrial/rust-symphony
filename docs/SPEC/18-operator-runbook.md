# 18 — Operator Runbook

Practical guide for running and operating the Symphony service. References **SPEC** and the implementation docs for configuration, observability, failure recovery, and security.

**See also:** [17-implementation-checklist.md](17-implementation-checklist.md), [12-logging-observability.md](12-logging-observability.md), [13-failure-recovery.md](13-failure-recovery.md), [14-security.md](14-security.md).

---

## Running the Service

### CLI and workflow path

- **Workflow path**: Pass an explicit path to `WORKFLOW.md` as a positional argument, or rely on the default `./WORKFLOW.md` (current working directory).
- **Startup**: The service validates config (workflow load, `tracker.repo`, `tracker.api_key`, `runner.command`) before entering the poll loop. If validation fails, the process exits with a nonzero status and an error message.
- **Normal operation**: Process runs until shutdown (e.g. SIGTERM). Exit 0 on clean shutdown; nonzero on startup failure or abnormal exit.

### Environment and secrets

- **Tracker token**: Set `GITHUB_TOKEN` (or the variable referenced in workflow front matter, e.g. `tracker.api_key: $GITHUB_TOKEN`). Required for fetching issues; validation fails if missing after `$VAR` resolution.
- **Workspace root**: Config value `workspace.root` supports `$VAR` and `~`; ensure the path exists and the process has read/write access.

---

## Observability

### Logs

- **Structured fields**: Logs include `issue_id`, `issue_identifier`, and `session_id` for issue and session context ([12-logging-observability.md](12-logging-observability.md)).
- **Level**: Use the configured env filter (e.g. `RUST_LOG`) to adjust verbosity. Default sink is stderr.
- **Sink failure**: If the log sink fails, the service logs a warning and continues; it does not crash.

### Optional status surface

- If the optional HTTP server or status dashboard is enabled, it is read-only and driven from orchestrator state. It does not affect dispatch or correctness.
- Endpoints (when enabled): e.g. `GET /`, `GET /api/v1/state`, `GET /api/v1/:issue_identifier`, `POST /api/v1/refresh` ([12-logging-observability.md](12-logging-observability.md) §13.7).

---

## Failure Recovery

### What the service does automatically

- **Dispatch validation failure**: Skips new dispatches for that tick; keeps running; continues reconciliation; logs and notifies.
- **Worker/session failure**: Removes the run from active state; applies exponential backoff and re-queues for retry per [07-polling-scheduling.md](07-polling-scheduling.md).
- **Tracker fetch failure**: Logs; skips dispatch that tick; next tick retries.
- **Config reload**: When `WORKFLOW.md` is edited, the loader re-reads and re-validates; in-flight sessions are not restarted.

### Restart and partial state

- Orchestrator state is **in-memory only**. On process restart:
  - Startup runs **terminal workspace cleanup** (e.g. issues in terminal states).
  - Poll loop starts; candidates are fetched and dispatched again. No restoration of previous retry timers or running sessions.

### Operator actions

| Situation | Action |
|----------|--------|
| Config or prompt change | Edit `WORKFLOW.md`; service reloads without restart (where supported). |
| Change issue state in tracker | Reconciliation stops runs (terminal → workspace cleanup; non-active → stop run). |
| Full reset or deployment | Restart the process; state is rebuilt from tracker and filesystem. |
| Stuck or misbehaving run | Change issue to a terminal state in the tracker, or restart the process. |

See [13-failure-recovery.md](13-failure-recovery.md) for failure classes and recovery behavior.

---

## Security and Safety

### Trust and secrets

- **API key**: Never log the resolved `tracker.api_key`; validation fails with a generic message if the secret is missing after `$VAR` resolution ([14-security.md](14-security.md)).
- **Trust boundary**: Document whether the environment is trusted-only or multi-tenant, and the chosen approval/sandbox policy for the coding agent.

### Workspace and hooks

- **Workspace root**: All issue workspaces live under `workspace.root`; paths are normalized and validated. Agent subprocess runs with `current_dir(workspace_path)` only ([08-worktree-management.md](08-worktree-management.md)).
- **Hooks**: Hooks are configured in `WORKFLOW.md` and run in the workspace dir with full shell; enforce timeout and truncate hook output in logs.

See [14-security.md](14-security.md) for filesystem safety, hook safety, and hardening options.

---

## Pre-production Checklist

- [ ] Run with valid credentials (e.g. `GITHUB_TOKEN`) and confirm candidate fetch and at least one dispatch cycle.
- [ ] Verify workflow path resolution and hook execution on the target OS/shell.
- [ ] If using the optional HTTP server: verify port and bind host (e.g. loopback) on the target environment.
- [ ] Confirm structured logs include expected fields and levels for debugging.

---

## References

- [SPEC.md](SPEC.md) — Full specification
- [17-implementation-checklist.md](17-implementation-checklist.md) — Definition of done
- [12-logging-observability.md](12-logging-observability.md) — Logging and status
- [13-failure-recovery.md](13-failure-recovery.md) — Failure model and recovery
- [14-security.md](14-security.md) — Security and operational safety
