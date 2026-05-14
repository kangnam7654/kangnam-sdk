use serde::{Deserialize, Serialize};

use crate::color::{Background, Color};
use crate::element::PptxElement;
use crate::geometry::{Emu, px_to_emu};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PptxDeck {
    /// Shown as the app title in PowerPoint. Optional.
    pub title: Option<String>,
    pub slides: Vec<PptxSlide>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PptxSlide {
    pub width_emu: Emu,
    pub height_emu: Emu,
    pub background: Background,
    pub elements: Vec<PptxElement>,
    /// PowerPoint speaker notes — emitted into `ppt/notesSlides/notesSlideN.xml`
    /// when present (Phase 6b-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_notes: Option<String>,
}

impl PptxSlide {
    /// Convenience: 1280×720 px (16:9) slide with a solid white background.
    pub fn blank_1280_720() -> Self {
        Self {
            width_emu: px_to_emu(1280.0),
            height_emu: px_to_emu(720.0),
            background: Background::Solid {
                color: Color::WHITE,
            },
            elements: Vec::new(),
            speaker_notes: None,
        }
    }
}

impl Default for PptxSlide {
    /// Default to a blank 16:9 slide so callers can use `..Default::default()`
    /// when they only want to set a subset of fields. Mirrors the Phase 6b
    /// addition of `speaker_notes` so existing callers don't need to update.
    fn default() -> Self {
        Self::blank_1280_720()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_slide_is_16x9_widescreen() {
        let s = PptxSlide::blank_1280_720();
        assert_eq!(s.width_emu, 12_192_000);
        assert_eq!(s.height_emu, 6_858_000);
        assert!(s.elements.is_empty());
    }

    #[test]
    fn deck_json_round_trip() {
        let d = PptxDeck {
            title: Some("demo".into()),
            slides: vec![PptxSlide::blank_1280_720()],
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: PptxDeck = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
