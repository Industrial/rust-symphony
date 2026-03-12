# 06 — Definition of “checks”

Specification: **SPEC_ADDENDUM_2 §B.6**.

---

## B.6 Definition of “checks”

- **Checks** means: GitHub Check Runs (e.g. GitHub Actions, third-party checks) and legacy Commit Statuses for the PR’s head commit.
- **Failed:** At least one check run or status has conclusion/state indicating failure (e.g. `failure`, `error`, `cancelled` as defined by the API). Implementations MUST document which values are treated as failed.
- **Pending / in progress:** Checks that are not yet completed (e.g. `queued`, `in_progress`, or `pending`). When all checks are pending or in progress and there is no qualifying mention, the orchestrator waits.
- **All passed:** All relevant checks have a successful conclusion. The orchestrator does **not** add any label (e.g. no “pr-complete”); the issue remains as-is until the human merges or takes other action.

---

## Implementation notes

- **symphony-tracker:** Normalize Check Runs and Commit Statuses into a small enum or struct (e.g. `CheckState::Pending | InProgress | Failed | Success`). Map GitHub API values to “failed” (e.g. `failure`, `error`, `cancelled`, `timed_out`) and “success” (e.g. `success`, `skipped` if desired). Document the mapping in this crate or in operator docs.
- **Aggregation:** “Any failed” → dispatch. “All success” or “all pending/in_progress” (and no mention) → wait.
