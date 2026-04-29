#![doc = include_str!("../README.md")]

#[cfg(feature = "slides")]
pub use canvas_slide_doc::{
    Background, Deck, Fill, Frame, ImageFit, ShapeKind, SiteDoc, SlideDoc, SlideElement, Stroke,
    TextAlign, TextStyle, CANVAS_HEIGHT, CANVAS_WIDTH,
};

#[cfg(feature = "slides")]
pub mod html {
    //! HTML rendering + DOM-manifest ingest + zone-override injection.
    //!
    //! Both submodule access (`html::html_render::render`) and flat
    //! re-exports (`html::render`) are supported so migrating callers can
    //! pick whichever reads best at the call site.
    pub use canvas_slide_doc::{html_render, inject, manifest_ingest};
    pub use canvas_slide_doc::html_render::*;
    pub use canvas_slide_doc::inject::*;
    pub use canvas_slide_doc::manifest_ingest::*;
}

#[cfg(feature = "llm")]
pub mod llm {
    //! AI provider abstraction + Gemini / Claude / LM Studio implementations.
    pub use canvas_llm::*;
}

#[cfg(feature = "editor")]
pub mod editor {
    //! LLM-driven generator + zone / section editors.
    pub use canvas_editor::*;
}

#[cfg(feature = "pptx-write")]
pub mod pptx {
    //! Editable PPTX export (pure Rust, no LibreOffice).
    pub use canvas_pptx_writer::*;
}
