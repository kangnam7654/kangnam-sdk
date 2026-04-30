//! Design-system catalog — DESIGN.md parser + token extractor.
//!
//! Builds on the [awesome-design-md][adm] 9-section schema (color · typography ·
//! spacing · layout · components · motion · voice · brand · anti-patterns)
//! and ships a 73-system vendored catalog at `systems/` (Linear, Stripe,
//! Vercel, Cursor, Apple, Anthropic, …) with the original Apache-2.0
//! attribution preserved at workspace `LICENSE-APACHE`.
//!
//! [adm]: https://github.com/VoltAgent/awesome-design-md

pub mod parser;
pub mod tokens;
pub mod loader;

pub use loader::{load_systems_from_dir, list_system_ids, DesignSystem};
pub use parser::{parse_design_md, NineSections, ParseError};
pub use tokens::{extract_color_tokens, ColorToken};
