# 08 — Interaction with base SPEC and Addendum 1

Specification: **SPEC_ADDENDUM_2 §B.8**.

---

## B.8 Interaction with base SPEC and Addendum 1

- **§8 Polling:** The same poll tick that fetches candidates and applies label filters (Addendum 1) also evaluates fix-PR candidates: for each issue with `pr_open_label` and `fix_pr` true, the orchestrator resolves the PR, fetches check status and (if configured) mentions, then decides wait vs dispatch. No separate “churn” or wait loop is required.
- **§1 Read-only:** The orchestrator does not add or remove labels or post comments. Adding `pr-complete` or any other label is **out of scope**; the orchestrator only reads.
- **Addendum 1 §A.2:** Single worker per issue still holds. A fix-PR dispatch is a re-dispatch for the same issue (same workspace, same branch); the issue remains excluded from the normal “unclaimed” candidate set by virtue of the claim label and/or pr-open label.
- **Addendum 1 §A.3:** The PR-driven handoff (open PR, comment, exit; human merges) is unchanged. This addendum only adds the option to re-dispatch when the PR needs fixes or when a human mentions the configured handle.

---

## Implementation notes

- **symphony-orchestration:** Integrate fix-PR evaluation into the existing poll handler: after building the normal candidate list (with Addendum 1 label filters), build the fix-PR candidate set (step 02), then for each fix-PR candidate resolve PR and fetch checks/mentions (steps 03–05); apply dispatch condition (step 04). Merge “to dispatch” issues with normal candidates respecting single-worker and concurrency.
- **Tests:** Ensure fix-PR path does not add labels or post comments (orchestrator only reads). Ensure an issue in `running` is not re-dispatched for fix-PR.
