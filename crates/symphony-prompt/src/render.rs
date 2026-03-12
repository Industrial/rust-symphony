//! Liquid template rendering (SPEC §12).

use liquid::ParserBuilder;
use liquid::ValueView;
use liquid::model::{Object, Value};
use symphony_domain::Issue;

use crate::PromptError;

/// Workflow context exposed to the prompt template (e.g. base branch for PRs).
#[derive(Debug, Clone)]
pub struct WorkflowPromptContext {
  /// Base branch for worker branches and PR target (e.g. main, develop).
  pub pr_base_branch: String,
}

/// Render prompt from template with issue, optional attempt, and optional workflow context.
pub fn render_prompt(
  template: &str,
  issue: &Issue,
  attempt: Option<u32>,
  workflow: Option<&WorkflowPromptContext>,
) -> Result<String, PromptError> {
  if template.trim().is_empty() {
    return Ok("You are working on an issue from GitHub.".to_string());
  }
  let parser = ParserBuilder::with_stdlib()
    .build()
    .map_err(|e| PromptError::TemplateParse(e.to_string()))?;
  let t = parser
    .parse(template)
    .map_err(|e| PromptError::TemplateParse(e.to_string()))?;

  let mut globals = Object::new();
  globals.insert("issue".into(), Value::Object(issue_to_liquid_object(issue)));
  globals.insert(
    "attempt".into(),
    attempt
      .map(|a| Value::scalar(a as i64))
      .unwrap_or(Value::Nil),
  );
  if let Some(w) = workflow {
    globals.insert(
      "workflow".into(),
      Value::Object(workflow_to_liquid_object(w)),
    );
  }
  let root = Value::Object(globals);
  let view = root
    .as_object()
    .ok_or_else(|| PromptError::TemplateRender("globals not object".into()))?;
  let out = t
    .render(view)
    .map_err(|e| PromptError::TemplateRender(e.to_string()))?;
  Ok(out)
}

fn workflow_to_liquid_object(w: &WorkflowPromptContext) -> Object {
  let mut obj = Object::new();
  obj.insert(
    "pr_base_branch".into(),
    Value::scalar(w.pr_base_branch.clone()),
  );
  obj
}

fn issue_to_liquid_object(issue: &Issue) -> Object {
  let mut obj = liquid::model::Object::new();
  obj.insert("id".into(), Value::scalar(issue.id.clone()));
  obj.insert("identifier".into(), Value::scalar(issue.identifier.clone()));
  obj.insert("title".into(), Value::scalar(issue.title.clone()));
  obj.insert("state".into(), Value::scalar(issue.state.clone()));
  if let Some(ref d) = issue.description {
    obj.insert("description".into(), Value::scalar(d.clone()));
  }
  if let Some(ref u) = issue.url {
    obj.insert("url".into(), Value::scalar(u.clone()));
  }
  let labels: Vec<Value> = issue
    .labels
    .iter()
    .map(|s| Value::scalar(s.clone()))
    .collect();
  obj.insert("labels".into(), Value::Array(labels));
  obj
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample_issue() -> Issue {
    Issue {
      id: "1".into(),
      identifier: "owner/repo#42".into(),
      title: "Fix the bug".into(),
      description: Some("Details".into()),
      priority: None,
      state: "open".into(),
      branch_name: None,
      url: None,
      labels: vec!["bug".into()],
      blocked_by: vec![],
      created_at: None,
      updated_at: None,
    }
  }

  #[test]
  fn render_prompt_empty_template() {
    let out = render_prompt("   ", &sample_issue(), None, None).unwrap();
    assert!(out.contains("GitHub"));
  }

  #[test]
  fn render_prompt_simple() {
    let out = render_prompt("Title: {{ issue.title }}", &sample_issue(), None, None).unwrap();
    assert_eq!(out, "Title: Fix the bug");
  }

  #[test]
  fn render_prompt_with_attempt() {
    let out = render_prompt("Attempt: {{ attempt }}", &sample_issue(), Some(2), None).unwrap();
    assert_eq!(out, "Attempt: 2");
  }

  #[test]
  fn render_prompt_with_workflow_pr_base_branch() {
    let workflow = WorkflowPromptContext {
      pr_base_branch: "main".to_string(),
    };
    let out = render_prompt(
      "Base: {{ workflow.pr_base_branch }}",
      &sample_issue(),
      None,
      Some(&workflow),
    )
    .unwrap();
    assert_eq!(out, "Base: main");
  }

  #[test]
  fn render_prompt_workflow_develop() {
    let workflow = WorkflowPromptContext {
      pr_base_branch: "develop".to_string(),
    };
    let out = render_prompt(
      "Branch: {{ workflow.pr_base_branch }}",
      &sample_issue(),
      None,
      Some(&workflow),
    )
    .unwrap();
    assert_eq!(out, "Branch: develop");
  }
}
