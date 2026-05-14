//! JSON prompt-template catalog for image/video generation models.
//!
//! Adapted from open-design's [`prompt-templates/`][odp] subtree. Each
//! template is a single JSON file describing a reusable prompt with
//! structured metadata (model · aspect · tags · category), preview URLs,
//! and **mandatory source attribution** for the underlying CC-BY-4.0 (and
//! similar) license terms — consumers MUST surface the [`SourceAttribution`]
//! when shipping the prompt to end users.
//!
//! Ships a 94-template vendored catalog at `templates/` (44 image + 50
//! video prompts) sourced from
//! [`YouMind-OpenLab/awesome-gpt-image-2`][gpt2],
//! [`YouMind-OpenLab/awesome-seedance-2-prompts`][sd2], and others.
//!
//! ```text
//! templates/
//! ├── image/         44 prompts (gpt-image-2 etc.)
//! └── video/         50 prompts (seedance-2.0 etc.)
//! ```
//!
//! [odp]: https://github.com/nexu-io/open-design/tree/main/prompt-templates
//! [gpt2]: https://github.com/YouMind-OpenLab/awesome-gpt-image-2
//! [sd2]:  https://github.com/YouMind-OpenLab/awesome-seedance-2-prompts

pub mod loader;
pub mod model;

pub use loader::{
    LoadError, list_template_ids, load_all_templates, load_template, load_templates_from_dir,
};
pub use model::{PromptTemplate, SourceAttribution, Surface};
