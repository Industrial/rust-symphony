//! Prompt render errors (SPEC §12.4).

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
  #[error("template parse error: {0}")]
  TemplateParse(String),

  #[error("template render error: {0}")]
  TemplateRender(String),
}
