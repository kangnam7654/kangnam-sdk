//! LLM-driven site (landing page) generator for the kangnam-sdk design
//! family. Composes `design-llm` + `design-doc-site`.

pub mod error;
pub mod site_generator;

pub use error::{EditorError, Result};
pub use site_generator::{SiteGenerationError, SiteGenerationEvent, SiteGenerator};
