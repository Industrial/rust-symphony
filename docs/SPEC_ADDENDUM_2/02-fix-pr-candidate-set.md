# 02 — Fix-PR candidate set

Specification: **SPEC_ADDENDUM_2 §B.2**.

---

## B.2 Fix-PR candidate set

- **Eligibility:** An issue is a **fix-PR candidate** when:
  1. `fix_pr` is `true`,
  2. The issue has the configured `tracker.pr_open_label` (Addendum 1),
  3. The issue is in an active state (per base SPEC §8),
  4. The issue is not already in `running` (no active agent for this issue),
  5. Concurrency and other base-SPEC dispatch rules allow a new run.

- Fix-PR candidates are considered **in addition to** (or as a special case of) the normal candidate set. The orchestrator MUST NOT dispatch the same issue twice; at most one worker per issue (Addendum 1 §A.2.2). For fix-PR, the worker re-uses the existing per-issue workspace and branch; it does not open a second PR.

- **Issue → PR resolution:** The orchestrator MUST resolve the pull request associated with the issue by a deterministic, configurable rule. Examples: (a) list PRs for the repo whose head branch matches the configured branch naming convention (e.g. `symphony/issue-<number>`), or (b) list PRs that reference the issue in the body or title (e.g. “Fixes #N”). The specification does not mandate a single method; the implementation MUST document the method used. If no PR is found for an issue that has the PR-open label, the orchestrator treats the issue as “wait” (do not dispatch) and MAY log.

---

## Implementation notes

- **symphony-tracker (or new module):** Implement “list PRs for issue” or “get PR by head branch pattern.” Document the resolution strategy (branch name vs “Fixes #N”).
- **symphony-orchestration:** Build fix-PR candidate set each tick when `fix_pr` is true: filter issues that have `pr_open_label`, are active, not in `running`, and pass concurrency; then resolve PR per issue. If no PR found, do not enqueue for fix-PR dispatch.
