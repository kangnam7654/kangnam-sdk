//! HTML rendering for `Deck` — extracted from the data crate so the
//! data-only `design-doc-slide` does not depend on the site rendering layer.

use kangnam_design_doc_slide::Deck;

use crate::html_render as slide_html;

/// Render the full deck as one standalone HTML document with N sections.
///
/// Each slide becomes a `<section class="slide" data-slide-id data-slide-index>`
/// so the navigator can `scrollIntoView` by slide_id and overrides can scope
/// by `section[data-slide-id="..."] [data-edit-zone="..."]`.
pub fn deck_to_html(deck: &Deck) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<!DOCTYPE html>\n<html lang=\"ko\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=1280\">\n");
    out.push_str("<title>Canvas Deck</title>\n");
    out.push_str("<style>\n");
    out.push_str(DECK_CSS);
    out.push_str("</style>\n</head>\n<body>\n");

    for (i, slide) in deck.slides.iter().enumerate() {
        out.push_str(&slide_html::render_section(slide, i));
        out.push('\n');
    }

    out.push_str("</body>\n</html>\n");
    out
}

/// CSS for the deck viewport — fixed-size sections + scroll-snap vertical so
/// the navigator's `scrollIntoView` snaps to slide boundaries. Background
/// for each slide is set on the `<section>` inline by `render_section`.
const DECK_CSS: &str = r#"
html, body { margin: 0; padding: 0; background: #f5f5f5; font-family: 'Pretendard', system-ui, -apple-system, sans-serif; }
* { box-sizing: border-box; }
body { scroll-snap-type: y mandatory; overflow-y: auto; height: 100vh; }
section.slide {
  position: relative;
  width: 1280px;
  height: 720px;
  margin: 0 auto;
  overflow: hidden;
  scroll-snap-align: start;
}
section.slide [data-edit-zone] { position: absolute; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_design_doc_slide::{
        Frame, SlideDoc, SlideElement, TextStyle, CANVAS_HEIGHT, CANVAS_WIDTH,
    };

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
    fn deck_to_html_emits_one_section_per_slide_with_slide_id() {
        let deck = Deck {
            id: "d1".into(),
            slides: vec![
                slide("slide-1", "one"),
                slide("slide-2", "two"),
                slide("slide-3", "three"),
            ],
        };
        let html = deck_to_html(&deck);
        assert_eq!(
            html.matches("<section").count(),
            3,
            "one <section> per slide, got html:\n{html}"
        );
        assert!(html.contains(r#"data-slide-id="slide-1""#));
        assert!(html.contains(r#"data-slide-id="slide-2""#));
        assert!(html.contains(r#"data-slide-id="slide-3""#));
        assert!(html.contains(r#"data-slide-index="0""#));
        assert!(html.contains(r#"data-slide-index="1""#));
        assert!(html.contains(r#"data-slide-index="2""#));
        assert!(html.contains("scroll-snap-type"));
        assert!(html.contains("scroll-snap-align"));
    }

    #[test]
    fn deck_to_html_dimensions_are_fixed_per_slide() {
        let deck = Deck::from_single(slide("s", "only"));
        let html = deck_to_html(&deck);
        assert!(html.contains(&format!("width: {CANVAS_WIDTH}px")));
        assert!(html.contains(&format!("height: {CANVAS_HEIGHT}px")));
    }

    #[test]
    fn deck_to_html_same_zone_id_across_sections_stays_distinct() {
        let deck = Deck {
            id: "d1".into(),
            slides: vec![
                slide("slide-1", "first title"),
                slide("slide-2", "second title"),
            ],
        };
        let html = deck_to_html(&deck);
        let zone_count = html.matches(r#"data-edit-zone="title""#).count();
        assert_eq!(zone_count, 2, "both sections keep their own 'title' zone");
    }
}
