# 01 — Label-based candidate filtering

Rust implementation notes for **SPEC_ADDENDUM_1 §A.1**. Extends the tracker client and config so candidate issues can be filtered by include/exclude labels. No new runtime crates required beyond those already used by the config and tracker layers.

**Deliverable:** Config keys `tracker.include_labels` and `tracker.exclude_labels` are parsed and exposed; the tracker client (or the layer that builds the candidate list) applies label filters after fetching by state. Unit tests must be written for all new code; implementation is not complete until tests are written and all tests pass. See [16-testing.md](../SPEC/16-testing.md) and [04-integration-and-config.md](04-integration-and-config.md).

---

## Crates

No new dependencies. Uses existing:

- **symphony-config**: Add optional `include_labels: Option<Vec<String>>` and `exclude_labels: Option<Vec<String>>` to the tracker config struct; parse from workflow front matter.
- **symphony-tracker**: Accept label filter options when fetching candidates; apply include then exclude after state filter (or in the same pass over the fetched list).
- **symphony-domain** (if present): Issue model already carries `labels`; no change required except that the tracker must populate it from the API so filters can be applied.

---

## A.1.1 Include labels (whitelist)

- **Config key:** `tracker.include_labels` (optional; list of strings in YAML).
- **Semantics:** If present and non-empty, an issue is a candidate only if it has **at least one** of the listed labels (case-sensitive or normalised per tracker; recommend normalising to lowercase for comparison if the tracker normalises label names).
- **If omitted or empty:** No include filter; do not drop any issue on account of labels at this step.

**Implementation:**

- In `symphony-config`, add `pub include_labels: Option<Vec<String>>` to the tracker config. Deserialize from `include_labels` in front matter.
- When building candidates: after fetching issues in active states, if `include_labels` is `Some(ref list)` and `list` is non-empty, retain only issues whose `labels` (or equivalent) contain at least one entry in `list`. Comparison: normalize both sides (e.g. lowercase) to match GitHub behaviour.

---

## A.1.2 Exclude labels (blacklist)

- **Config key:** `tracker.exclude_labels` (optional; list of strings).
- **Semantics:** If present and non-empty, an issue is **not** a candidate if it has **any** of the listed labels.
- **If omitted or empty:** No exclude filter.

**Implementation:**

- In `symphony-config`, add `pub exclude_labels: Option<Vec<String>>` to the tracker config.
- When building candidates: after applying include_labels (if any), drop any issue whose labels intersect `exclude_labels`. Normalise for comparison as above.

---

## A.1.3 Order of application

1. Fetch issues by active state (per base SPEC §8; existing behaviour).
2. Apply **include_labels** if configured: drop issues that have none of the include labels.
3. Apply **exclude_labels** if configured: drop issues that have any of the exclude labels.
4. Then apply the rest of candidate selection (not in `running`, not in `claimed`, slots, etc.) per base SPEC §8.2.

Apply filters in the tracker client (or in a thin layer that the orchestrator uses) so that the orchestrator receives an already-filtered candidate list. Do not hardcode label names; read all from config.

---

## A.1.4 Configuration

All label names are configurable in the workflow. Implementations MUST NOT hardcode label names; they MUST be read from the workflow config.

---

## Tests (must be written and pass)

- **Config:** Parse workflow front matter with `include_labels` and `exclude_labels` (YAML arrays); assert values round-trip. Omitted keys → `None`. Empty arrays → `Some(vec![])`; filter logic should treat empty include as “no filter”.
- **Filter logic:** Unit tests with a small list of mock issues (with `labels`). Given `include_labels = ["a", "b"]`, retain only issues that have at least one of `a` or `b`. Given `exclude_labels = ["x"]`, drop issues that have `x`. Combined: include then exclude; assert final list.
- **Tracker integration:** If the tracker client fetches from a mocked API (e.g. wiremock), stub responses that include `labels` on each issue; assert that after applying config-driven include/exclude, only the expected issues are returned. Edge cases: empty labels on issue; duplicate labels; case normalization.
- All tests must pass before the step is considered complete.
