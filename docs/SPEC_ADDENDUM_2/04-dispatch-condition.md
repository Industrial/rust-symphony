# 04 — Dispatch condition (when to run the agent)

Specification: **SPEC_ADDENDUM_2 §B.4**.

---

## B.4 Dispatch condition (when to run the agent)

- For a fix-PR candidate issue with a resolved PR, the orchestrator MUST **dispatch** (enqueue for agent run) if **either** of the following is true:

  1. **Check failed:** At least one check run or commit status for the PR’s head is in a **failed** state (or equivalent; implementation MUST define how “failed” is determined from the API).
  2. **Mention:** `tracker.mention_handle` is configured and a **qualifying mention** exists (see B.5). A qualifying mention triggers dispatch even if checks are pending or have passed; it allows a human to request work explicitly.

- The orchestrator MUST **wait** (do not dispatch) if:
  - No PR could be resolved for the issue, or
  - All checks are in a non-failed state (pending, in_progress, or success) **and** there is no qualifying mention.

- **No AI while waiting:** The agent is only started when the dispatch condition above is met. Polling the GitHub API for check status and comments does not involve the coding agent; it avoids unnecessary agent (AI) cost until a fix or a human-requested reaction is needed.

---

## Implementation notes

- **symphony-orchestration:** After fetching check status and (if configured) mentions for a fix-PR candidate, evaluate: dispatch if `any_check_failed || has_qualifying_mention`; otherwise skip (wait). Enqueue only when dispatch is true; do not start the agent process until then.
- **Tests:** Unit or integration test: when checks are pending or success and no mention, no dispatch. When check failed or qualifying mention present, dispatch (mock tracker responses).
