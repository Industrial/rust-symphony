# 03 — Single polling loop: check status and mentions

Specification: **SPEC_ADDENDUM_2 §B.3**.

---

## B.3 Single polling loop: check status and mentions

- The orchestrator uses a **single polling loop**. On each tick, for each fix-PR candidate issue (and after resolving the PR as in B.2), the orchestrator **reads** (no writes) from the tracker and from the GitHub API:

  1. **Check runs and commit status** for the PR’s head commit (e.g. GitHub Checks API and/or commit status API).
  2. **Optionally**, if `tracker.mention_handle` is set: **issue comments** and **PR comments** (including review comments) to detect mentions of the configured handle (e.g. `@symphony`).

- The orchestrator MUST NOT add or remove labels or post comments. It only reads.

---

## Implementation notes

- **symphony-tracker:** Add (or extend) client to:
  - Fetch check runs for a commit ref: `GET /repos/{owner}/{repo}/commits/{ref}/check-runs` (and/or commit statuses).
  - When `mention_handle` is set: fetch issue comments, PR review comments (and optionally PR body) for the issue/PR.
- **symphony-orchestration:** In the same poll tick that processes normal candidates, iterate fix-PR candidates; for each, resolve PR then call tracker to get check status (and mentions if configured). No separate loop or timer.
