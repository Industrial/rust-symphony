# 03 — PR-driven workflow

Rust implementation notes for **SPEC_ADDENDUM_1 §A.3**. The PR-driven workflow is primarily a **behavioural contract** for the coding agent (branch, open PR, comment, exit; human merges). The orchestrator does not merge PRs or “wait” long-running; implementation focuses on config and prompt/workflow documentation.

**Deliverable:** Config key `tracker.pr_open_label` (optional) is parsed and exposed; workflow/prompt instructions document the PR-driven flow (single branch per issue, open PR, comment, exit; do not merge). Unit tests for config parsing; acceptance or doc tests for workflow behaviour where applicable. Implementation is not complete until tests are written and all tests pass. See [16-testing.md](../SPEC/16-testing.md) and [04-integration-and-config.md](04-integration-and-config.md).

---

## Crates

No new dependencies. Uses existing:

- **symphony-config**: Add optional `pr_open_label: Option<String>` to the tracker config; parse from workflow front matter.
- **symphony-workflow / symphony-prompt**: Prompt template or WORKFLOW.md body should instruct the agent to: use one branch per issue, open a PR when ready, add a comment with the PR link, then exit; do not merge the PR. Branch naming can be configurable (e.g. `symphony/issue-{{ issue.number }}`) in the prompt or a dedicated config key if desired.
- **symphony-runner**: No change to runner logic; “exit after opening PR” is agent behaviour, not a special runner mode.

---

## A.3.1–A.3.4 Branch, PR, comment, exit

- **Branch:** Single branch per issue; naming convention (e.g. `symphony/issue-<number>`) can be documented in the workflow or prompt. Worker commits on that branch in the per-issue workspace.
- **PR:** Agent opens a PR from that branch to the default branch; PR body SHOULD include “Fixes #N” (or equivalent) so merging closes the issue.
- **Comment:** Agent posts a comment on the issue with the PR link and short summary.
- **Exit:** Agent exits successfully. The issue keeps the claim label (and optionally `pr_open_label`), so it stays excluded from candidates. No long-running “wait for merge” process.

**Implementation:**

- No orchestrator code for “waiting.” Ensure prompt construction (symphony-prompt) can include instructions for PR-driven flow when the workflow opts in (e.g. via a flag or a standard prompt fragment). Document in WORKFLOW.md or in a workflow template that when using claim_label + PR-driven flow, the agent must add claim label, do the work, open PR, comment, then exit.

---

## A.3.6 Optional: PR-open label

- **Config key:** `tracker.pr_open_label` (optional; string).
- **Semantics:** If present, the agent MAY add this label when it has opened a PR. This label SHOULD be in `exclude_labels` so “PR open, waiting for merge” is not re-dispatched.

**Implementation:**

- In `symphony-config`, add `pub pr_open_label: Option<String>` to the tracker config. Deserialize from `pr_open_label` in front matter.
- Workflow prompt can reference this label by name (from config or from a literal in the prompt) so the agent knows which label to add. Orchestrator does not add it; filtering is already handled by exclude_labels when the label is present on the issue.

---

## Tests (must be written and pass)

- **Config:** Parse workflow front matter with `pr_open_label` (string); assert value; omitted → `None`.
- **Prompt / workflow:** If there is a code path that injects PR-driven instructions into the prompt (e.g. when a config flag is set), unit test that the injected text contains the expected guidance (branch, PR, comment, exit; do not merge). If PR-driven flow is entirely in static prompt text, a doc test or acceptance checklist in 04-integration-and-config is sufficient.
- All tests must pass before the step is considered complete.
