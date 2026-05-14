//! SiteDoc: canvas-web payload — a landing page composed of vertically
//! stacked sections. Mirrors [`kangnam_design_doc_slide::deck::Deck`] structurally (id +
//! children) so the zone-editing/override machinery built for decks can
//! be reused, but individual sections are `SlideDoc`s rendered inline
//! (no 1280×720 viewport, no absolute positioning).
//!
//! Each section has a semantic `kind` (hero / features / testimonial /
//! stats / cta / faq / footer / custom) which drives prompt templates
//! and default styling; the `kind` is stored on the `SlideDoc`'s `title`
//! field as a human label and encoded in the `id` prefix so parsing is
//! stable across generations.

use serde::{Deserialize, Serialize};

use kangnam_design_doc_slide::slide::{SlideDoc, SlideElement};

/// Root payload for a canvas-web project version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SiteDoc {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub sections: Vec<SlideDoc>,
}

impl SiteDoc {
    /// Wrap a single `SlideDoc` into a single-section site — convenient
    /// for tests and for early generations that only yield one section
    /// (e.g. a bare hero).
    pub fn from_single(doc: SlideDoc) -> Self {
        let id = format!("site-{}", doc.id);
        Self {
            id,
            title: None,
            sections: vec![doc],
        }
    }

    /// Look up a section by its `SlideDoc::id`.
    pub fn find_section(&self, section_id: &str) -> Option<&SlideDoc> {
        self.sections.iter().find(|s| s.id == section_id)
    }

    /// Mutable variant of [`find_section`] — used when applying a
    /// zone-level edit to a specific section's elements.
    pub fn find_section_mut(&mut self, section_id: &str) -> Option<&mut SlideDoc> {
        self.sections.iter_mut().find(|s| s.id == section_id)
    }

    /// Render the site as a standalone HTML document. Sections stack
    /// vertically (no fixed height) and each carries `data-section-id`
    /// so the iframe-based canvas can map clicks back to a section.
    /// Tailwind and Pretendard are loaded via CDN so the AI-generated
    /// inner HTML (which uses Tailwind utility classes) renders without
    /// a build step.
    pub fn to_html(&self) -> String {
        let mut out = String::with_capacity(4096);
        out.push_str("<!DOCTYPE html>\n<html lang=\"ko\">\n<head>\n");
        out.push_str("<meta charset=\"utf-8\">\n");
        out.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n");
        out.push_str("<title>");
        out.push_str(&html_escape(self.title.as_deref().unwrap_or("Canvas Web")));
        out.push_str("</title>\n");
        out.push_str("<script src=\"https://cdn.tailwindcss.com\"></script>\n");
        out.push_str(
            "<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/gh/orioncactus/pretendard@v1.3.9/dist/web/variable/pretendardvariable.css\">\n",
        );
        out.push_str(
            "<style>body{font-family:'Pretendard Variable',system-ui,sans-serif;margin:0;}</style>\n",
        );
        out.push_str("</head>\n<body>\n");
        for section in &self.sections {
            out.push_str(&format!(
                "<section data-section-id=\"{}\">\n",
                html_escape(&section.id)
            ));
            out.push_str(&render_section_elements(section));
            out.push_str("\n</section>\n");
        }
        out.push_str("</body>\n</html>\n");
        out
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Web-mode element rendering: no absolute positioning, no slide
/// viewport. Element `content` is AI-authored Tailwind-classed HTML; we
/// just wrap it with a `data-edit-zone` attribute so the zone editor
/// can click into it.
fn render_section_elements(section: &SlideDoc) -> String {
    let mut out = String::new();
    for el in &section.elements {
        match el {
            SlideElement::Text { id, content, .. } => {
                out.push_str(&format!(
                    "<div data-edit-zone=\"{}\">{}</div>",
                    html_escape(id),
                    content
                ));
            }
            SlideElement::Image { id, src, .. } => {
                out.push_str(&format!(
                    "<img data-edit-zone=\"{}\" src=\"{}\">",
                    html_escape(id),
                    html_escape(src)
                ));
            }
            SlideElement::Shape { id, .. } => {
                out.push_str(&format!(
                    "<div data-edit-zone=\"{}\"></div>",
                    html_escape(id)
                ));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_design_doc_slide::slide::{Frame, SlideDoc, SlideElement, TextStyle};

    fn text_el(id: &str, content: &str) -> SlideElement {
        SlideElement::Text {
            id: id.into(),
            frame: Frame {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 20.0,
            },
            content: content.into(),
            style: TextStyle::default(),
        }
    }

    #[test]
    fn from_single_wraps_slide_doc() {
        let doc = SlideDoc::empty("hero-1");
        let site = SiteDoc::from_single(doc);
        assert_eq!(site.sections.len(), 1);
        assert_eq!(site.id, "site-hero-1");
        assert!(site.title.is_none());
    }

    #[test]
    fn find_section_returns_matching_section() {
        let site = SiteDoc {
            id: "s1".into(),
            title: None,
            sections: vec![SlideDoc::empty("a"), SlideDoc::empty("b")],
        };
        assert_eq!(site.find_section("b").unwrap().id, "b");
        assert!(site.find_section("c").is_none());
    }

    #[test]
    fn find_section_mut_lets_us_mutate_in_place() {
        let mut site = SiteDoc {
            id: "s1".into(),
            title: None,
            sections: vec![SlideDoc::empty("hero-1")],
        };
        let section = site.find_section_mut("hero-1").unwrap();
        section.title = Some("Hero".into());
        assert_eq!(site.sections[0].title.as_deref(), Some("Hero"));
    }

    #[test]
    fn roundtrip_json_preserves_sections_and_title() {
        let site = SiteDoc {
            id: "s1".into(),
            title: Some("Landing".into()),
            sections: vec![SlideDoc::empty("hero-1"), SlideDoc::empty("cta-1")],
        };
        let json = serde_json::to_string(&site).unwrap();
        let back: SiteDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(site, back);
    }

    #[test]
    fn roundtrip_without_title_is_symmetric() {
        let site = SiteDoc::from_single(SlideDoc::empty("hero-1"));
        let json = serde_json::to_string(&site).unwrap();
        // `title: None` should be skipped on serialize — deserialize still fills it.
        assert!(!json.contains("\"title\""));
        let back: SiteDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(site, back);
    }

    #[test]
    fn to_html_stacks_sections_with_data_section_id() {
        let site = SiteDoc {
            id: "s1".into(),
            title: Some("Landing".into()),
            sections: vec![SlideDoc::empty("hero-1"), SlideDoc::empty("cta-1")],
        };
        let html = site.to_html();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains(r#"data-section-id="hero-1""#));
        assert!(html.contains(r#"data-section-id="cta-1""#));
        assert!(html.contains("cdn.tailwindcss.com"));
        assert!(html.contains("pretendardvariable.css"));
        // stacked — no per-slide 1280×720 viewport leaks in.
        assert!(!html.contains("height: 720px"));
        assert!(!html.contains("width: 1280px"));
    }

    #[test]
    fn to_html_renders_text_elements_with_edit_zone_attribute() {
        let mut section = SlideDoc::empty("hero-1");
        section.elements.push(text_el(
            "headline",
            "<h1 class=\"text-5xl font-bold\">Welcome</h1>",
        ));
        let site = SiteDoc {
            id: "s1".into(),
            title: None,
            sections: vec![section],
        };
        let html = site.to_html();
        assert!(html.contains(r#"data-edit-zone="headline""#));
        assert!(html.contains("Welcome"));
        // AI-authored content should pass through untouched (we don't
        // double-escape it — only attributes are escaped).
        assert!(html.contains("text-5xl font-bold"));
    }

    #[test]
    fn to_html_escapes_section_ids_and_titles() {
        let site = SiteDoc {
            id: "s1".into(),
            title: Some("A & B".into()),
            sections: vec![SlideDoc::empty("he\"ro")],
        };
        let html = site.to_html();
        assert!(html.contains("A &amp; B"));
        assert!(html.contains(r#"data-section-id="he&quot;ro""#));
    }
}
