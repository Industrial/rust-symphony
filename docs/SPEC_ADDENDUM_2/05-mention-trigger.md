# 05 — Mention trigger

Specification: **SPEC_ADDENDUM_2 §B.5**.

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

## Implementation notes

- **symphony-config:** Add `mention_handle: Option<String>` to tracker config; parse from workflow front matter.
- **symphony-tracker:** When fetching comments, return only those that (a) contain `@{mention_handle}` in the body and (b) satisfy the newness cutoff. Cutoff can be: PR `updated_at`, or orchestrator-provided “last dispatch time” / “last seen comment id” per issue.
- **symphony-orchestration:** Maintain per-issue state (in-memory or lightweight) for “last reacted comment id” or “last dispatch time” if using that variant of the newness rule. Document chosen rule in code or operator docs.
