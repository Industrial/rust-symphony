# Symphony Service Specification — Addendum 1

**Status:** Draft  
**Supersedes / extends:** SPEC.md (base specification)  
**Purpose:** Label-based candidate filtering, durable single-worker claim via labels, and PR-driven workflow. All behaviour is configurable via `WORKFLOW.md` front matter.

---

## A.1 Label-based candidate filtering

The tracker configuration MAY include optional label filters. These are applied when building the candidate set (e.g. after fetching issues by active state and before applying in-memory `running` / `claimed` checks).

### A.1.1 Include labels (whitelist)

- **Config key:** `tracker.include_labels` (optional; list of strings).
- **Semantics:** If present, an issue is a candidate only if it has **at least one** of the listed labels.
- **If omitted:** No include filter; all issues that pass state and other filters remain candidates (subject to exclude).

### A.1.2 Exclude labels (blacklist)

- **Config key:** `tracker.exclude_labels` (optional; list of strings).
- **Semantics:** If present, an issue is **not** a candidate if it has **any** of the listed labels.
- **If omitted:** No exclude filter.

### A.1.3 Order of application

1. Fetch issues by active state (per base SPEC §8).
2. Apply **include_labels** if configured: drop issues that have none of the include labels.
3. Apply **exclude_labels** if configured: drop issues that have any of the exclude labels.
4. Then apply the rest of candidate selection (not in `running`, not in `claimed`, slots available, etc.) per §8.2.

### A.1.4 Configuration

All label names are configurable in the workflow. Implementations MUST NOT hardcode label names; they MUST be read from the workflow config so that different repositories can use different conventions (e.g. `symphony`, `symphony-claimed`, or `bot/in-progress`).

---

## A.2 Durable claim and single-worker semantics

A **claim** is the guarantee that at most one worker is assigned to an issue at a time and that this assignment survives process restarts. Claim is represented by a **label** on the issue.

### A.2.1 Claim label

- **Config key:** `tracker.claim_label` (optional; string).
- **Semantics:** The label that the coding agent MUST add to the issue when it “claims” the issue (typically as its first step). This label SHOULD be listed in `tracker.exclude_labels` so that once added, the issue is no longer a candidate and no other worker (and no re-dispatch after restart) will pick it up.
- **Orchestrator:** The orchestrator remains **read-only** with respect to the tracker. It does not add or remove labels. The coding agent adds the claim label using whatever tools it has (e.g. GitHub CLI, API) as instructed by the workflow prompt.

### A.2.2 Single worker per issue

- Only issues that do **not** have the claim label (and that pass include/exclude and other rules) are candidates.
- Once a worker adds the claim label, the issue is excluded on all subsequent polls and after any orchestrator restart. No persistent orchestrator state is required; the label on the issue is the durable claim.
- Worker identity (e.g. process ID, hostname) is **not** encoded in the label. A single configurable label is sufficient: “this issue is taken.”

### A.2.3 Restarts

- After an orchestrator or worker restart, the orchestrator fetches candidates and applies label filters. Any issue that still has the claim label remains excluded. There is no need to “reassign” to the same worker; the issue is simply not re-dispatched until the label is removed (e.g. by a human to re-queue) or the issue is closed (terminal state).

### A.2.4 Re-queuing

- To allow the issue to be picked up again (e.g. after a failed run or to retry), a human (or an external process) may remove the claim label. The issue then becomes a candidate again subject to include/exclude and other rules.

---

## A.3 PR-driven workflow

The workflow MAY define a PR-driven handoff: the worker implements the task on a branch, opens a pull request, comments on the issue, then exits. The worker does not merge the PR; a human merges, and the issue is closed (e.g. via “Fixes #N” in the PR).

### A.3.1 Branch and work

- The worker works in the per-issue workspace (e.g. a git worktree). It MUST use a single branch per issue (e.g. `symphony/issue-<number>` or a configurable naming convention). All changes are committed on that branch.

### A.3.2 Pull request

- When implementation is ready, the worker MUST open a pull request from that branch to the default branch. The PR body SHOULD reference the issue (e.g. “Fixes #N”) so that merging the PR will close the issue (tracker behaviour).
- The worker MUST NOT merge the PR. Merging is performed by a human.

### A.3.3 Comment on issue

- The worker SHOULD add a comment on the issue with the PR link and a short summary (e.g. “Opened PR #42; waiting for review and CI.”).

### A.3.4 “Waiting” and exit

- After opening the PR and posting the comment, the worker’s job is complete. The worker exits successfully. “Waiting” for review and merge is represented by **not re-dispatching** the issue: the issue retains the claim label (and optionally a “PR open” label if configured), so it remains excluded from the candidate set. The orchestrator does not keep a long-running process for “waiting”; the process is deterministic and short-lived per run.

### A.3.5 When the human merges

- The human reviews and merges the PR. The tracker (e.g. GitHub) closes the issue when the PR is merged (when “Fixes #N” is used). The issue enters a terminal state; the orchestrator’s existing behaviour (terminal-state handling, startup cleanup) applies. No additional specification is required.

### A.3.6 Optional: PR-open label

- **Config key:** `tracker.pr_open_label` (optional; string).
- **Semantics:** If present, the agent MAY add this label when it has opened a PR. This label SHOULD be in `exclude_labels` so that “PR open, waiting for merge” is not re-dispatched. Use of this label is for visibility and filtering; the claim label alone is sufficient to prevent re-dispatch if it is never removed until the issue is closed.

---

## A.4 Interaction with base SPEC

- **§8 Polling and candidate selection:** Label filters (include_labels, exclude_labels) are an additional layer applied when building the candidate list. All other rules in §8.2 (state, `running`, `claimed`, slots) continue to apply after label filtering.
- **§8.4 Retry and backoff:** When processing due retries, the orchestrator re-fetches issue state. If the issue now has an exclude label (e.g. claim label), it is no longer candidate-eligible; the orchestrator releases the claim (removes from in-memory `claimed` / retry state) and does not re-dispatch, consistent with “no longer eligible.”
- **§1 and tracker read-only:** The orchestrator still only reads the tracker. Adding or removing labels is done by the coding agent (or by humans/external tools), not by the orchestrator.
- **§9 Workspace:** Per-issue workspace and branch naming are unchanged; the addendum only specifies that the worker uses a single branch per issue and opens a PR from that branch.

---

## A.5 Summary of new config keys

| Key | Section | Type | Purpose |
|-----|---------|------|---------|
| `tracker.include_labels` | A.1.1 | optional list of strings | Candidate must have at least one of these labels. |
| `tracker.exclude_labels` | A.1.2 | optional list of strings | Candidate must have none of these labels. |
| `tracker.claim_label` | A.2.1 | optional string | Label the agent adds when claiming; should be in exclude_labels. |
| `tracker.pr_open_label` | A.3.6 | optional string | Optional label when PR is open; should be in exclude_labels if used. |

All keys are optional. When absent, behaviour matches the base SPEC (no label-based filtering, no claim label semantics).
