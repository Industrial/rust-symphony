# 07 — Agent behaviour when dispatched for fix-PR

Specification: **SPEC_ADDENDUM_2 §B.7**.

---

## B.7 Agent behaviour when dispatched for fix-PR

- When the orchestrator dispatches an issue under this addendum, the agent runs in the **existing** per-issue workspace and on the **existing** branch (the one associated with the open PR). The agent MUST:
  - Pull or rebase as appropriate, then make changes to address the failure or the human request (e.g. review comment),
  - Commit and push to the same branch,
  - Exit when done (no need to open a new PR or add labels unless the workflow prompt instructs otherwise).

- The workflow prompt MAY instruct the agent to add a comment on the issue or PR when it has pushed fixes (e.g. “Pushed fixes for CI.”). That is agent behaviour, not orchestrator behaviour.

---

## Implementation notes

- **symphony-runner:** No change to runner lifecycle. The same “run agent in workspace” path is used; the workspace and branch are already the ones for the open PR.
- **symphony-prompt / WORKFLOW.md:** When fix_pr is enabled, the prompt template SHOULD include instructions for fix-PR runs: e.g. “This run is to fix an existing PR (CI failed or a human requested changes). Use the current branch, fix the problem, commit and push. Do not open a new PR.” Optional: instruct the agent to post a short comment after pushing (e.g. “Pushed fixes.”). Prompt can be conditional on a variable (e.g. `fix_pr_run` or context indicating “fix existing PR”) if the runner passes that through.
