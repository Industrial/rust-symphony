//! WorkflowDefinition (SPEC §4.1.2).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// YAML front matter as a generic map (further parsed by config layer).
    pub config: serde_json::Value,
    /// Markdown body after front matter, trimmed.
    pub prompt_template: String,
}
