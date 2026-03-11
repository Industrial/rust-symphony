# 02 — Durable claim and single-worker semantics

Rust implementation notes for **SPEC_ADDENDUM_1 §A.2**. The orchestrator remains read-only; claim is represented by a configurable label that the **coding agent** adds to the issue. The implementation extends config and ensures the claim label is used in exclude filtering.

**Deliverable:** Config key `tracker.claim_label` is parsed and exposed; documentation and/or workflow prompt instruct the agent to add this label when claiming; `claim_label` is included in `exclude_labels` (or applied as exclude) so claimed issues are not re-dispatched. Unit tests must be written for all new code; implementation is not complete until tests are written and all tests pass. See [16-testing.md](../SPEC/16-testing.md) and [04-integration-and-config.md](04-integration-and-config.md).

---

## Crates

No new dependencies. Uses existing:

- **symphony-config**: Add optional `claim_label: Option<String>` to the tracker config; parse from workflow front matter.
- **symphony-tracker**: No change to write path (orchestrator does not add/remove labels). Candidate filtering already uses `exclude_labels` (step 01); workflow or operator must ensure `claim_label` is in `exclude_labels` when using claim semantics.
- **symphony-runner / orchestration**: No orchestrator code that writes to the tracker. Retry and reconciliation behaviour (release claim when issue has exclude label or is terminal) is as in addendum A.4.

---

## A.2.1 Claim label

- **Config key:** `tracker.claim_label` (optional; string).
- **Semantics:** The label that the coding agent MUST add to the issue when it “claims” the issue (typically as its first step). This label SHOULD be listed in `tracker.exclude_labels` so that once added, the issue is no longer a candidate.
- **Orchestrator:** The orchestrator remains **read-only**. It does not add or remove labels. The coding agent adds the claim label using tools (e.g. GitHub CLI, API) as instructed by the workflow prompt.

**Implementation:**

- In `symphony-config`, add `pub claim_label: Option<String>` to the tracker config. Deserialize from `claim_label` in front matter.
- Optionally: if `claim_label` is set and not already present in `exclude_labels`, append it when building the effective exclude list (so that a single config of `claim_label: symphony-claimed` guarantees exclusion without requiring the operator to duplicate it in `exclude_labels`). Document this in 04-integration-and-config.
- WORKFLOW.md (or prompt template) must instruct the agent to add the claim label at the start of work; that is workflow content, not orchestrator code.

---

## A.2.2 Single worker per issue

- Only issues that do **not** have the claim label (and that pass include/exclude and other rules) are candidates. This is enforced by having the claim label in `exclude_labels` (or in the effective exclude list).
- No worker ID in the label; one label means “this issue is taken.”

No additional orchestrator logic: candidate selection already excludes issues with any exclude label. No persistent orchestrator state for claim; the label on the issue is the durable claim.

---

## A.2.3 Restarts and re-queuing

- After restart, the orchestrator fetches candidates and applies label filters; issues that still have the claim label remain excluded.
- Re-queuing: a human (or external process) removes the claim label; the issue becomes a candidate again. No implementation change required beyond correct exclude filtering.

---

## Tests (must be written and pass)

- **Config:** Parse workflow front matter with `claim_label` (string); assert value; omitted → `None`.
- **Effective exclude list:** If implementation merges `claim_label` into effective exclude labels when set, unit test: given `exclude_labels = ["a"]` and `claim_label = Some("b")`, effective exclude list contains both `a` and `b`; given `claim_label = None`, effective exclude list is only `exclude_labels`.
- **Integration:** With mock tracker returning issues with/without the claim label, assert that issues with the claim label never appear in the candidate list after filtering. All tests must pass before the step is considered complete.
