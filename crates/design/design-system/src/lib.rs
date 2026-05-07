//! Design-system catalog — DESIGN.md parser + token extractor.
//!
//! Builds on the [awesome-design-md][adm] 9-section schema (color · typography ·
//! spacing · layout · components · motion · voice · brand · anti-patterns)
//! and ships a 138-system vendored catalog at `systems/` covering both the
//! brand axis (Linear, Stripe, Vercel, Cursor, Apple, Anthropic, …) and the
//! design-genre axis (brutalism, glassmorphism, claymorphism, neumorphism,
//! editorial, dithered, …) — sourced from [VoltAgent/awesome-design-md][adm]
//! and [bergside/awesome-design-skills][ads] plus four hand-authored OD
//! starters (`default`, `warm-editorial`, `atelier-zero`, `kami`). Original
//! Apache-2.0/MIT attributions preserved at workspace `LICENSE-APACHE` and
//! the catalog [`README.md`](systems/README.md).
//!
//! [adm]: https://github.com/VoltAgent/awesome-design-md
//! [ads]: https://github.com/bergside/awesome-design-skills

pub mod parser;
pub mod tokens;
pub mod loader;

pub use loader::{load_systems_from_dir, list_system_ids, DesignSystem};
pub use parser::{parse_design_md, NineSections, ParseError};
pub use tokens::{extract_color_tokens, ColorToken};
