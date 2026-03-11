# 04 — Workflow Specification (Repository Contract)

Rust implementation notes for **SPEC §5**. Uses **serde_yaml** for YAML; front matter split via a **library** (preferred) or **regex**; **file watching** is optional (notify/notify-debouncer or defer).

**Deliverable:** Unit tests must be written for all code (e.g. path resolution, loader, error types); implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Crates

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
# Front matter: use one of the following
yaml-front-matter = "0.1"   # Preferred: parses --- ... --- and deserializes with serde
# markdown-frontmatter = "0.2"  # Alternative: supports YAML/TOML/JSON delimiters
# If no front-matter crate: use regex to split, then serde_yaml::from_str on the first block

# Optional: file watching for dynamic reload (SPEC §6.2)
# notify = "6"
# notify-debouncer-mini = "0.4"
```

- **serde_yaml**: Parse YAML front matter content into `serde_json::Value` or into typed config structs (see [05-configuration.md](05-configuration.md)).
- **Front matter**: Prefer **yaml-front-matter** (or **markdown-frontmatter**) to split `---` … `---` from the body; otherwise use **regex** to find the first `---` and the next `---`, then parse the slice between with `serde_yaml`.
- **File watching**: SPEC requires re-reading `WORKFLOW.md` on change. Implementations can use **notify** / **notify-debouncer-mini** for that, or poll the file mtime; choice is implementation-defined.

---

## 5.1 File Discovery and Path Resolution (SPEC §5.1)

- **Precedence**: (1) Explicit path from CLI/runtime (e.g. `std::env::args` or config), (2) default `WORKFLOW.md` in current working directory (`std::env::current_dir()`).
- **Behavior**: If the path cannot be read, return a typed error `missing_workflow_file` (e.g. `enum WorkflowError { MissingWorkflowFile(PathBuf), ... }`).

```rust
use std::path::PathBuf;

pub fn resolve_workflow_path(explicit: Option<PathBuf>) -> Result<PathBuf, WorkflowError> {
    let path = match explicit {
        Some(p) => p,
        None => std::env::current_dir().map_err(|e| WorkflowError::Io(e))?
            .join("WORKFLOW.md"),
    };
    if path.exists() && path.is_file() {
        Ok(path)
    } else {
        Err(WorkflowError::MissingWorkflowFile(path))
    }
}
```

---

## 5.2 File Format (SPEC §5.2)

- **Format**: Markdown with optional YAML front matter. If the file starts with `---`, the first block between `---` and the next `---` is YAML; the rest is the prompt body (trimmed).
- **Rules**:
  - No leading `---` → entire file is prompt body; config = empty map.
  - YAML must decode to a **map** (object); non-map YAML → `workflow_front_matter_not_a_map`.
  - Prompt body = remaining content after the closing `---`, trimmed.

### Option A: Library (yaml-front-matter)

```rust
use serde::Deserialize;
use yaml_front_matter::YamlFrontMatter;

use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct RawConfig(#[serde(flatten)] HashMap<String, serde_yaml::Value>);

pub fn load_workflow(content: &str) -> Result<WorkflowDefinition, WorkflowError> {
    let doc = YamlFrontMatter::parse::<RawConfig>(content)
        .map_err(|e| WorkflowError::WorkflowParseError(e.to_string()))?;
    let config = doc.metadata
        .map(|c| c.0.into_iter().collect())
        .unwrap_or_default();
    let prompt_template = doc.content.trim().to_string();
    Ok(WorkflowDefinition { config: serde_json::to_value(config).unwrap(), prompt_template })
}
```

(Adjust to your `WorkflowDefinition`: `config` as `serde_json::Value` or a map; ensure YAML map is required when front matter is present.)

### Option B: Regex split + serde_yaml

```rust
use regex::Regex;
use serde_yaml::Value;

fn split_front_matter(content: &str) -> (Option<&str>, &str) {
    static RE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| {
        Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---\r?\n(.*)").unwrap()
    });
    if let Some(caps) = RE.captures(content) {
        let yaml = caps.get(1).map(|m| m.as_str());
        let body = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        (yaml, body)
    } else {
        (None, content.trim())
    }
}

pub fn load_workflow(content: &str) -> Result<WorkflowDefinition, WorkflowError> {
    let (yaml_opt, body) = split_front_matter(content);
    let config = match yaml_opt {
        None => serde_json::Value::Object(Default::default()),
        Some(yaml) => {
            let v: Value = serde_yaml::from_str(yaml)
                .map_err(|e| WorkflowError::WorkflowParseError(e.to_string()))?;
            if !v.is_mapping() {
                return Err(WorkflowError::WorkflowFrontMatterNotAMap);
            }
            serde_json::to_value(v).map_err(|e| WorkflowError::WorkflowParseError(e.to_string()))?
        }
    };
    Ok(WorkflowDefinition { config, prompt_template: body.to_string() })
}
```

---

## 5.3 Front Matter Schema (SPEC §5.3)

The **config** in `WorkflowDefinition` is the front matter as a map. The **config layer** (see [05-configuration.md](05-configuration.md)) deserializes it into typed structs for `tracker`, `polling`, `workspace`, `hooks`, `agent`, and `runner`. Unknown keys are ignored.

- Top-level keys: `tracker`, `polling`, `workspace`, `hooks`, `agent`, `runner` (+ optional `server`, etc.).
- **runner** configures the coding agent CLI (any provider: Codex, Cursor, Claude, OpenCode, etc.).

---

## 5.4 Prompt Template Contract (SPEC §5.4)

The **prompt_template** string is the Markdown body. It is rendered with a strict template engine (Liquid-compatible semantics) with variables `issue` and `attempt`; see [11-prompt-construction.md](11-prompt-construction.md). Unknown variables/filters must fail rendering.

- Empty body: runtime may use a minimal default prompt (e.g. `You are working on an issue from GitHub.`).
- Parse/render failures are configuration or run-attempt errors, not silent fallbacks.

---

## 5.5 Workflow Validation and Error Surface (SPEC §5.5)

Error types (recommended):

- `missing_workflow_file`
- `workflow_parse_error` (YAML or front matter split)
- `workflow_front_matter_not_a_map`
- `template_parse_error` / `template_render_error` (during prompt rendering; see doc 11)

Dispatch gating: Workflow file / YAML errors block new dispatches until fixed; template errors fail only the affected run attempt.

---

## File Watching (SPEC §6.2, optional)

Dynamic reload is required: when `WORKFLOW.md` changes, re-read and re-apply config and prompt without restart.

- **Option 1**: **notify** + **notify-debouncer-mini** — watch the workflow file path, debounce, then call the same loader and apply the new `WorkflowDefinition` to the config layer.
- **Option 2**: Poll file mtime on each tick or on a timer.
- **Option 3**: Defer: document that file watching is TBD and that defensive re-read before dispatch is still required.

Invalid reloads must not crash the service; keep the last known good config and emit an operator-visible error.

---

## References

- [SPEC.md](SPEC.md) §5 — Workflow Specification  
- [05-configuration.md](05-configuration.md) — Config layer, defaults, env, validation  
- [11-prompt-construction.md](11-prompt-construction.md) — Template rendering
