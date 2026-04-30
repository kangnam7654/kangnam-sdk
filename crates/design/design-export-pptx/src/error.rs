use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PptxWriteError {
    #[error("invalid image bytes for slide {slide_idx} element {element_idx}: {msg}")]
    InvalidImage { slide_idx: usize, element_idx: usize, msg: String },

    #[error("image mime mismatch: declared {declared}, detected {detected}")]
    MimeMismatch { declared: &'static str, detected: &'static str },

    #[error("empty deck — need at least one slide")]
    EmptyDeck,

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("xml encoding error: {0}")]
    Xml(String),

    /// Loaded zip does not look like a PPTX (missing required OOXML parts).
    #[error("invalid pptx template ({reason}): {detail}")]
    InvalidTemplate { reason: String, detail: String },

    /// Layout index out of range (1-based).
    #[error("layout {layout_num} not found (max {max})")]
    LayoutNotFound { layout_num: usize, max: usize },

    /// Layout has no `<p:ph idx="..."/>` matching the requested index.
    #[error("placeholder idx={placeholder_idx} not found in layout {layout_num}")]
    PlaceholderNotFound { layout_num: usize, placeholder_idx: u32 },

    /// `SlideRef` does not correspond to an appended slide.
    #[error("slide {slide_num} not found")]
    SlideNotFound { slide_num: usize },
}

pub type Result<T> = std::result::Result<T, PptxWriteError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_deck_displays_human_readable() {
        let err = PptxWriteError::EmptyDeck;
        assert_eq!(format!("{err}"), "empty deck — need at least one slide");
    }

    #[test]
    fn io_error_is_from_convertible() {
        let io: std::io::Error = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        let err: PptxWriteError = io.into();
        assert!(matches!(err, PptxWriteError::Io(_)));
    }
}
