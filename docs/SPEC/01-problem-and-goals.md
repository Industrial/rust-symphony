# 01 — Problem Statement and Goals

Rust implementation notes for **SPEC §1–2**.

---

## 1. Problem Statement (SPEC §1)

Symphony is a long-running automation service that:

- Reads work from **GitHub Issues**
- Creates an **isolated workspace** per issue
- Runs a **coding agent session** for that issue inside the workspace

### Four operational problems solved

| Problem | Implementation implication |
|--------|----------------------------|
| Repeatable daemon workflow | Single long-lived process; poll loop + bounded concurrency (see [07-polling-scheduling.md](07-polling-scheduling.md)). |
| Per-issue workspace isolation | Workspace manager maps issue identifier → directory; agent is always spawned with that directory as `cwd` (see [08-workspace-management.md](08-workspace-management.md)). |
| Versioned workflow policy | `WORKFLOW.md` in repo: YAML front matter + prompt body; loaded and watched at runtime (see [04-workflow-spec.md](04-workflow-spec.md), [05-configuration.md](05-configuration.md)). |
| Observability for concurrent runs | Structured logging + optional status/dashboard; per-issue and per-session context in logs (see [12-logging-observability.md](12-logging-observability.md)). |

### Boundaries

- **Symphony**: scheduler, runner, tracker **reader**. No first-class tracker write APIs in the orchestrator.
- **Ticket writes**: Done by the coding agent via tools (e.g. optional GitHub client-side tool).
- **Success**: Reaching a workflow-defined handoff state (e.g. “Human Review”), not necessarily “Done”.

---

## 2. Goals and Non-Goals (SPEC §2)

### 2.1 Goals — implementation mapping

| Goal | Rust / doc |
|------|-------------|
| Poll on fixed cadence, bounded concurrency | Tokio interval + semaphore/slots; [07-polling-scheduling.md](07-polling-scheduling.md). |
| Single authoritative orchestrator state | In-memory struct(s); one owner; [06-orchestration.md](06-orchestration.md), [07-polling-scheduling.md](07-polling-scheduling.md). |
| Deterministic per-issue workspaces, preserved across runs | Workspace path = `f(workspace_root, sanitize(issue_identifier))`; [08-workspace-management.md](08-workspace-management.md). |
| Stop runs when issue state becomes ineligible | Reconciliation every tick; fetch current states for running issue IDs; [07-polling-scheduling.md](07-polling-scheduling.md), [10-github-tracker.md](10-github-tracker.md). |
| Transient failure recovery with exponential backoff | Retry queue with backoff formula; [07-polling-scheduling.md](07-polling-scheduling.md), [13-failure-recovery.md](13-failure-recovery.md). |
| Load behavior from `WORKFLOW.md` | Workflow loader + config layer; [04-workflow-spec.md](04-workflow-spec.md), [05-configuration.md](05-configuration.md). |
| Operator-visible observability (at least structured logs) | `tracing` (or chosen logging crate) with `issue_id` / `issue_identifier` / `session_id`; [12-logging-observability.md](12-logging-observability.md). |
| Restart recovery without persistent DB | Tracker-driven: on startup, fetch terminal issues and clean workspaces; re-poll and dispatch; no DB; [07-polling-scheduling.md](07-polling-scheduling.md), [13-failure-recovery.md](13-failure-recovery.md). |

### 2.2 Non-Goals

- No rich web UI or multi-tenant control plane (optional simple dashboard is acceptable).
- No prescribed dashboard/terminal UI implementation.
- Not a general workflow engine or distributed job scheduler.
- No built-in business logic for *how* to edit tickets/PRs/comments (that lives in workflow prompt + agent tools).
- No mandated sandbox/approval posture; implementation documents its own.

---

## References

- [SPEC.md](SPEC.md) §1 — Problem Statement  
- [SPEC.md](SPEC.md) §2 — Goals and Non-Goals
