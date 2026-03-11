//! Prompt construction with Liquid (SPEC §12).
//!
//! See `docs/11-prompt-construction.md`.

mod error;
mod render;

pub use error::PromptError;
pub use render::render_prompt;
