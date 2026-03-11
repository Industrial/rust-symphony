//! Prompt construction with Liquid (SPEC §12).
//!
//! See `docs/11-prompt-construction.md`.

#![allow(clippy::missing_docs_in_private_items)]

mod error;
mod render;

pub use error::PromptError;
pub use render::render_prompt;
