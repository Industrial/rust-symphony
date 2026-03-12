# 09 — Summary of new config keys

Specification: **SPEC_ADDENDUM_2 §B.9**.

---

## B.9 Summary of new config keys

| Key | Section | Type | Purpose |
|-----|---------|------|---------|
| `fix_pr` | B.1 | optional boolean; default `false` | When true, enable fix-PR behaviour for issues with pr_open_label. |
| `tracker.mention_handle` | B.5 | optional string | Handle to look for in comments (e.g. `symphony` → `@symphony`). If set, a qualifying mention triggers dispatch in addition to “check failed”. |

When `fix_pr` is false or omitted, this addendum has no effect. When `fix_pr` is true, the orchestrator uses the single polling loop and read-only API calls described in B.2–B.6 to decide when to dispatch.

---

## Implementation notes

- **symphony-config:** Ensure both keys are parsed and exposed; `fix_pr` at top level, `mention_handle` under `tracker`. Defaults: `fix_pr = false`, `mention_handle = None`.
- **Definition of done:** All steps 01–08 implemented and tested; config summary matches implementation; orchestrator remains read-only; no new labels added by the orchestrator.
