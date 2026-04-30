//! Site document model — landing page composed of vertically stacked
//! sections, plus HTML rendering, DOM-manifest ingest, and zone override
//! injection.
//!
//! Builds on `design-doc-slide`: each section is a `SlideDoc` rendered
//! inline (no 1280×720 viewport, no absolute positioning).

pub mod site;
pub mod html_render;
pub mod manifest_ingest;
pub mod inject;
pub mod deck_html;

pub use site::SiteDoc;
pub use deck_html::deck_to_html;
