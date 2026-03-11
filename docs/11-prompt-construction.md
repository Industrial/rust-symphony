# 11 — Prompt Construction and Context Assembly

Rust implementation notes for **SPEC §12**. Uses the **liquid** crate for Liquid-compatible templating with strict variable checking; **serde** to build the `issue` object for the template.

---

## Crates

```toml
[dependencies]
liquid = "0.26"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- **liquid**: Liquid-compatible templates; strict mode so unknown variables/filters fail. Pass a **liquid::Object** (or equivalent) with `issue` and optional `attempt`.
- **serde_json**: Build a JSON value for `issue` from domain `Issue` so the template can iterate `issue.labels`, `issue.blocked_by`, etc.

---

## 12.1 Inputs (SPEC §12.1)

- **workflow.prompt_template**: String (from [04-workflow-spec.md](04-workflow-spec.md)).
- **issue**: Normalized `Issue`; convert to a structure the template engine accepts (e.g. liquid `Object` or a JSON-like map with nested arrays for labels and blocked_by).
- **attempt**: `Option<u32>` or `Option<i64>`; null/absent for first run, integer for retry or continuation.

---

## 12.2 Rendering Rules (SPEC §12.2)

- **Strict variables**: Enable strict mode in liquid so undefined variables cause a render error.
- **Strict filters**: Unknown filters error.
- **Issue shape**: Convert `Issue` to a map with string keys; nested `labels` (array of strings) and `blocked_by` (array of objects with `id`, `identifier`, `state`) so templates can iterate.

Example conversion (conceptual): `serde_json::to_value(issue)?` then into liquid’s data type, or build liquid::Object from the struct. Ensure nested arrays/maps are preserved.

---

## 12.3 Retry/Continuation Semantics (SPEC §12.3)

Pass `attempt` into the template so the prompt can branch (e.g. “This is a retry” or “Continue from the previous session”). Use `attempt: None` for first run, `Some(n)` for retries/continuation.

---

## 12.4 Failure Semantics (SPEC §12.4)

If rendering fails (parse error, unknown variable, unknown filter): return a typed error (e.g. `TemplateParseError`, `TemplateRenderError`). The agent runner fails the run attempt; the orchestrator treats it like any other worker failure and applies retry logic.

**Empty template**: If the prompt body is empty, the runtime may substitute a minimal default (e.g. “You are working on an issue from GitHub.”) instead of rendering.

---

## References

- [SPEC.md](SPEC.md) §12 — Prompt Construction  
- [04-workflow-spec.md](04-workflow-spec.md) — prompt_template  
- [03-domain-model.md](03-domain-model.md) — Issue
