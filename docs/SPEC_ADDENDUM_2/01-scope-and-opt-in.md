# 01 — Scope and opt-in

Specification: **SPEC_ADDENDUM_2 §B.1**.

---

## B.1 Scope and opt-in

- **Config key:** `fix_pr` (optional; boolean).
- **Default:** `false`.
- **Semantics:** When `true`, the orchestrator applies the fix-PR logic described in this addendum for issues that have the PR-open label (see Addendum 1 §A.3.6). When `false`, behaviour is unchanged: issues with `pr_open_label` remain excluded from dispatch (per Addendum 1); no check-status or mention polling is performed.
- Implementations MUST NOT enable fix-PR behaviour unless the workflow explicitly sets `fix_pr` to `true`.

---

## Implementation notes

- **symphony-config:** Parse top-level `fix_pr` from workflow front matter; default to `false` when omitted. Expose as e.g. `fix_pr: bool`.
- **symphony-runner / orchestration:** Gate all fix-PR logic (candidate set, check fetch, mention fetch, dispatch) on `fix_pr == true`. When false, do not call PR or Checks API for fix-PR purposes.
