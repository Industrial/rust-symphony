# Symphony Service Specification — Addendum 2

**Status:** Draft  
**Supersedes / extends:** SPEC.md, SPEC_ADDENDUM_1.md  
**Purpose:** Optional “fix PR” behaviour: when an issue has the PR-open label, the orchestrator may re-dispatch the agent to fix a failing PR (or react to a mention) instead of only waiting for human merge. Orchestrator remains read-only; a single polling loop decides wait vs dispatch by reading GitHub check status and optional mention triggers. No new labels are added by the orchestrator.

---

## B.1 Scope and opt-in

- **Config key:** `fix_pr` (optional; boolean).
- **Default:** `false`.
- **Semantics:** When `true`, the orchestrator applies the fix-PR logic described in this addendum for issues that have the PR-open label (see Addendum 1 §A.3.6). When `false`, behaviour is unchanged: issues with `pr_open_label` remain excluded from dispatch (per Addendum 1); no check-status or mention polling is performed.
- Implementations MUST NOT enable fix-PR behaviour unless the workflow explicitly sets `fix_pr` to `true`.

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

## B.3 Single polling loop: check status and mentions

- The orchestrator uses a **single polling loop**. On each tick, for each fix-PR candidate issue (and after resolving the PR as in B.2), the orchestrator **reads** (no writes) from the tracker and from the GitHub API:

  1. **Check runs and commit status** for the PR’s head commit (e.g. GitHub Checks API and/or commit status API).
  2. **Optionally**, if `tracker.mention_handle` is set: **issue comments** and **PR comments** (including review comments) to detect mentions of the configured handle (e.g. `@symphony`).

- The orchestrator MUST NOT add or remove labels or post comments. It only reads.

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

## B.5 Mention trigger

- **Config key:** `tracker.mention_handle` (optional; string). Example: `"symphony"`.
- **Semantics:** If present, the orchestrator fetches issue comments and (as needed) PR review/comments. A **qualifying mention** is a comment whose body contains the substring `@<mention_handle>` (e.g. `@symphony`), subject to the newness rule below.
- **If omitted:** Only “check failed” can trigger dispatch; mention-based dispatch is disabled.

### B.5.1 Newness rule (avoid re-dispatch on the same comment)

- A mention MUST be considered only if it is **new** relative to the last time the orchestrator could have reacted. Implementations MUST use one of the following (or an equivalent documented rule):
  - Comments created **after** the PR’s last update (e.g. `updated_at` of the PR), or
  - Comments created **after** the last dispatch (or last agent run) for this issue, using in-memory or lightweight persisted state (e.g. “last seen comment id” or “last dispatch time” per issue).

- This prevents the same old comment from triggering dispatch on every poll. The specification does not require a persistent database; in-memory state that resets on orchestrator restart is acceptable, with the consequence that after a restart an old mention might trigger one more dispatch unless the implementation uses another cutoff (e.g. PR `updated_at`).

---

## B.6 Definition of “checks”

- **Checks** means: GitHub Check Runs (e.g. GitHub Actions, third-party checks) and legacy Commit Statuses for the PR’s head commit.
- **Failed:** At least one check run or status has conclusion/state indicating failure (e.g. `failure`, `error`, `cancelled` as defined by the API). Implementations MUST document which values are treated as failed.
- **Pending / in progress:** Checks that are not yet completed (e.g. `queued`, `in_progress`, or `pending`). When all checks are pending or in progress and there is no qualifying mention, the orchestrator waits.
- **All passed:** All relevant checks have a successful conclusion. The orchestrator does **not** add any label (e.g. no “pr-complete”); the issue remains as-is until the human merges or takes other action.

---

## B.7 Agent behaviour when dispatched for fix-PR

- When the orchestrator dispatches an issue under this addendum, the agent runs in the **existing** per-issue workspace and on the **existing** branch (the one associated with the open PR). The agent MUST:
  - Pull or rebase as appropriate, then make changes to address the failure or the human request (e.g. review comment),
  - Commit and push to the same branch,
  - Exit when done (no need to open a new PR or add labels unless the workflow prompt instructs otherwise).

- The workflow prompt MAY instruct the agent to add a comment on the issue or PR when it has pushed fixes (e.g. “Pushed fixes for CI.”). That is agent behaviour, not orchestrator behaviour.

---

## B.8 Interaction with base SPEC and Addendum 1

- **§8 Polling:** The same poll tick that fetches candidates and applies label filters (Addendum 1) also evaluates fix-PR candidates: for each issue with `pr_open_label` and `fix_pr` true, the orchestrator resolves the PR, fetches check status and (if configured) mentions, then decides wait vs dispatch. No separate “churn” or wait loop is required.
- **§1 Read-only:** The orchestrator does not add or remove labels or post comments. Adding `pr-complete` or any other label is **out of scope**; the orchestrator only reads.
- **Addendum 1 §A.2:** Single worker per issue still holds. A fix-PR dispatch is a re-dispatch for the same issue (same workspace, same branch); the issue remains excluded from the normal “unclaimed” candidate set by virtue of the claim label and/or pr-open label.
- **Addendum 1 §A.3:** The PR-driven handoff (open PR, comment, exit; human merges) is unchanged. This addendum only adds the option to re-dispatch when the PR needs fixes or when a human mentions the configured handle.

---

## B.9 Summary of new config keys

| Key | Section | Type | Purpose |
|-----|---------|------|---------|
| `fix_pr` | B.1 | optional boolean; default `false` | When true, enable fix-PR behaviour for issues with pr_open_label. |
| `tracker.mention_handle` | B.5 | optional string | Handle to look for in comments (e.g. `symphony` → `@symphony`). If set, a qualifying mention triggers dispatch in addition to “check failed”. |

When `fix_pr` is false or omitted, this addendum has no effect. When `fix_pr` is true, the orchestrator uses the single polling loop and read-only API calls described in B.2–B.6 to decide when to dispatch.
