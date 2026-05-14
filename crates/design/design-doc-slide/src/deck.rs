//! Canvas v2 multi-slide deck — a thin wrapper around `Vec<SlideDoc>` that
//! preserves every v1 invariant per slide (1280×720, SlideElement schema,
//! `data-edit-zone` attribution, PPTX/HTML render path) while lifting the
//! deck to first-class. Legacy single-slide versions read as `Deck::from_single`.

use serde::{Deserialize, Serialize};

use crate::slide::SlideDoc;

/// Container for N slides rendered as one presentation. `id` is the deck's
/// own identifier, distinct from the individual `SlideDoc.id` values which
/// remain the primary key for zone-override + edit RPCs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Deck {
    pub id: String,
    pub slides: Vec<SlideDoc>,
}

impl Deck {
    /// Wrap a single SlideDoc so v1 rows read transparently as a 1-slide deck.
    pub fn from_single(doc: SlideDoc) -> Self {
        // Deck id is distinct from the slide id — we don't want them to
        // collide when the same version is queried two ways.
        let id = format!("deck-{}", doc.id);
        Self {
            id,
            slides: vec![doc],
        }
    }

    pub fn find_slide(&self, slide_id: &str) -> Option<&SlideDoc> {
        self.slides.iter().find(|s| s.id == slide_id)
    }

    pub fn find_slide_mut(&mut self, slide_id: &str) -> Option<&mut SlideDoc> {
        self.slides.iter_mut().find(|s| s.id == slide_id)
    }
}

// HTML rendering for `Deck` lives in `design-doc-site::deck_html` so the
// data-only crate has no dependency on the rendering layer. Generator
// prompt templates and DB-coupled `deck_from_version` helpers live in the
// kangnam-client app (they depend on `design-editor-slide` and
// `models::project::Version` respectively).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide::{Frame, SlideElement, TextStyle};

    fn slide(id: &str, content: &str) -> SlideDoc {
        let mut doc = SlideDoc::empty(id);
        doc.elements.push(SlideElement::Text {
            id: "title".into(),
            frame: Frame {
                x: 80.0,
                y: 80.0,
                w: 1120.0,
                h: 120.0,
            },
            content: content.into(),
            style: TextStyle::default(),
        });
        doc
    }

    #[test]
    fn from_single_wraps_one_slide_and_preserves_it() {
        let doc = slide("slide-a", "표지");
        let deck = Deck::from_single(doc.clone());
        assert_eq!(deck.slides.len(), 1);
        assert_eq!(deck.slides[0], doc);
        assert_ne!(deck.id, doc.id, "deck id must not collide with slide id");
    }

    #[test]
    fn find_slide_returns_matching_and_none_otherwise() {
        let deck = Deck {
            id: "d1".into(),
            slides: vec![slide("slide-1", "a"), slide("slide-2", "b")],
        };
        assert_eq!(deck.find_slide("slide-2").unwrap().id, "slide-2");
        assert!(deck.find_slide("missing").is_none());
    }

    #[test]
    fn find_slide_mut_allows_editing_in_place() {
        let mut deck = Deck {
            id: "d1".into(),
            slides: vec![slide("slide-1", "old")],
        };
        let s = deck.find_slide_mut("slide-1").unwrap();
        s.title = Some("바뀐 제목".into());
        assert_eq!(deck.slides[0].title.as_deref(), Some("바뀐 제목"));
    }

    #[test]
    fn deck_json_round_trip_preserves_structure() {
        let deck = Deck {
            id: "deck-1".into(),
            slides: vec![slide("s1", "one"), slide("s2", "two"), slide("s3", "three")],
        };
        let json = serde_json::to_string(&deck).unwrap();
        let parsed: Deck = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, deck);
    }
}
