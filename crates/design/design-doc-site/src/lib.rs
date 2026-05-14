//! Site document model — landing page composed of vertically stacked
//! sections, plus HTML rendering, DOM-manifest ingest, and zone override
//! injection.
//!
//! Builds on `design-doc-slide`: each section is a `SlideDoc` rendered
//! inline (no 1280×720 viewport, no absolute positioning).

pub mod deck_html;
pub mod html_render;
pub mod inject;
pub mod manifest_ingest;
pub mod model;
pub mod site;

pub use deck_html::deck_to_html;
pub use model::{
    Manifest, ManifestColor, ManifestError, ManifestRect, ManifestShape, ManifestText,
};
pub use site::SiteDoc;
