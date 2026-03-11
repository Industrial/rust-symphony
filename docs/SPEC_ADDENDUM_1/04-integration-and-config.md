# 04 — Integration with base SPEC and config summary

Rust implementation notes for **SPEC_ADDENDUM_1 §A.4 and §A.5**. This step documents how addendum behaviour integrates with the base SPEC (polling, retry, read-only tracker) and provides a single reference for all new config keys and for the definition of done (tests, crates).

**Deliverable:** Behavioural alignment with base SPEC is implemented and tested; retry and reconciliation treat exclude-labelled issues as ineligible; config key summary is the source of truth. Unit tests must be written for any integration logic; implementation is not complete until tests are written and all tests pass. See [16-testing.md](../SPEC/16-testing.md).

---

## Crates (summary for addendum)

| Crate | Role for addendum |
|-------|--------------------|
| **symphony-config** | `include_labels`, `exclude_labels`, `claim_label`, `pr_open_label`; all optional, from workflow front matter. |
| **symphony-tracker** | Apply include/exclude label filters when building candidate list; optionally merge `claim_label` into effective exclude list. |
| **symphony-runner / symphony-orchestration** | Use filtered candidate list; retry and reconciliation use re-fetched state (labels and state); release claim when issue is no longer eligible (terminal or has exclude label). |
| **symphony-workflow / symphony-prompt** | Prompt can reference PR-driven flow; no new crates. |

No new external dependencies beyond those already in the base SPEC implementation.

---

## A.4 Interaction with base SPEC

### §8 Polling and candidate selection

- Label filters (include_labels, exclude_labels) are an **additional layer** applied when building the candidate list (after fetch by active state, before in-memory `running` / `claimed` and slot checks). Implement in the tracker client or in the component that returns candidates to the orchestrator.
- All other rules in §8.2 (state, not in running, not in claimed, slots, blocker rule) continue to apply **after** label filtering.

### §8.4 Retry and backoff

- When processing due retries, the orchestrator **re-fetches** issue state (existing behaviour). If the issue now has an **exclude label** (e.g. claim label added by the agent), it is no longer candidate-eligible: release the in-memory claim (remove from `claimed` / retry state) and **do not** re-dispatch.
- Same as “issue not found” or “issue terminal”: release and do not re-dispatch. Implement by re-running the same candidate-eligibility logic (state + label filters) on the re-fetched issue.

### §1 and tracker read-only

- The orchestrator **only reads** the tracker. Adding or removing labels is done by the coding agent (or by humans/external tools), not by the orchestrator. No new write operations in the tracker client for the orchestrator.

### §9 Workspace

- Per-issue workspace and branch naming are unchanged. The addendum only specifies that the worker uses a single branch per issue and opens a PR from that branch; that is agent behaviour, not a change to workspace layout.

---

## A.5 Config key summary

| Key | Type | Purpose |
|-----|------|---------|
| `tracker.include_labels` | optional list of strings | Candidate must have at least one of these labels. |
| `tracker.exclude_labels` | optional list of strings | Candidate must have none of these labels. |
| `tracker.claim_label` | optional string | Label the agent adds when claiming; must be in effective exclude list (in exclude_labels or auto-merged). |
| `tracker.pr_open_label` | optional string | Optional label when PR is open; should be in exclude_labels if used. |

All keys are optional. When absent, behaviour matches the base SPEC (no label-based filtering, no claim semantics).

---

## Definition of done (addendum steps 01–04)

- [x] **Step 01 — Label filtering:** Config keys parsed; include/exclude applied in candidate build; unit tests for config and filter logic; tracker integration tests with mocked issues; all tests pass.
- [x] **Step 02 — Durable claim:** Config key `claim_label` parsed; effective exclude list includes claim_label when set; unit tests; integration test that claimed issues are not re-dispatched; all tests pass.
- [x] **Step 03 — PR-driven workflow:** Config key `pr_open_label` parsed; workflow/prompt documents PR-driven flow; unit tests for config and any prompt injection; all tests pass.
- [x] **Step 04 — Integration:** Retry/reconciliation treat exclude-labelled issues as ineligible (unit or integration test); config summary matches implementation; all tests pass.

---

## References

- [SPEC_ADDENDUM_1.md](../SPEC_ADDENDUM_1.md) — Addendum specification
- [SPEC.md](../SPEC.md) — Base specification
- [16-testing.md](../SPEC/16-testing.md) — Test and validation matrix
