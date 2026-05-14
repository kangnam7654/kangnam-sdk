//! Minimal HTML → PptxDeck conversion (Phase 6d).
//!
//! ## Scope
//!
//! High-fidelity HTML-to-PPTX (matching CSS layout, computed styles,
//! flex/grid, shadows, gradients, typography metrics) requires a real
//! browser engine — that's huashu-design's `html2pptx.js` running on
//! Playwright. Porting that to pure Rust isn't feasible; the path
//! forward is one of:
//!
//! - **(a) Sidecar Node/Playwright** — call `playwright` from a Tauri
//!   command that pipes the HTML in and reads structured DOM data
//!   back out. Pros: huashu parity. Cons: Node + Chromium dep on the
//!   user's machine.
//! - **(b) Tauri webview hidden tab + JS bridge** — render the HTML
//!   into a hidden webview tab, run `getBoundingClientRect()` +
//!   `getComputedStyle()` over the DOM, ship the result back via
//!   `__TAURI__.event.emit`. Pros: pure Rust + bundled webview.
//!   Cons: more bespoke code.
//!
//! Both options are out of scope for v1 — see ADR-013 (`docs/adr/`)
//! for the decision record.
//!
//! ## What this module ships
//!
//! A *very* minimal HTML reader that extracts what it can without a
//! browser engine:
//! - Slide size (from `<body data-slide-w="…" data-slide-h="…">`)
//! - Solid background color (from `<body style="background:#…">`)
//! - One TextBox per top-level `<h1>` / `<h2>` / `<p>` (positioned
//!   via simple block-stacking heuristics)
//! - Embedded `<img src="data:image/…">` elements (data URIs only —
//!   non-data URIs aren't fetched)
//!
//! This is enough for the closed-loop demo + as a fixture target for
//! future browser-backed implementations. Real layout-faithful
//! conversions need (a) or (b).

use crate::color::{Background, Color, Fill};
use crate::deck::{PptxDeck, PptxSlide};
use crate::element::{
    ImageBox, ImageFit, ImageMime, PptxElement, ShapeBox, ShapeKind, TextAlign, TextBox, TextStyle,
};
use crate::geometry::{Frame, px_to_emu};

/// Convert an HTML string into a single-slide PptxDeck.
///
/// The HTML body must be wrapped in a `<body>` tag (or a single
/// section behaves as the slide). Multi-slide HTML is left for the
/// browser-backed implementation.
pub fn from_html(html: &str) -> PptxDeck {
    let (width_px, height_px) = extract_slide_size(html).unwrap_or((1280, 720));
    let background = extract_body_background(html)
        .map(|c| Background::Solid { color: c })
        .unwrap_or(Background::Solid {
            color: Color::WHITE,
        });

    let mut elements: Vec<PptxElement> = Vec::new();

    // Stack text blocks vertically with simple 60px increments — far
    // from accurate layout, but gives a usable preview for tests.
    let mut y_cursor: f32 = 60.0;
    let lines = extract_text_blocks(html);
    for block in &lines {
        let style = TextStyle {
            font_family: "Inter".into(),
            font_size_pt: block.size_pt,
            font_weight: block.weight,
            color: Color::from_hex("111827").unwrap_or(Color::WHITE),
            align: TextAlign::Left,
            line_height: 1.4,
            letter_spacing_pt: 0.0,
            ..Default::default()
        };
        elements.push(PptxElement::Text(TextBox {
            frame: Frame::from_px(
                60.0,
                y_cursor,
                (width_px as f32) - 120.0,
                block.size_pt * 1.6,
            ),
            content: block.text.clone(),
            style,
        }));
        y_cursor += block.size_pt * 1.8;
    }

    // Embedded images via data URIs.
    for img in extract_data_uri_images(html) {
        elements.push(PptxElement::Image(ImageBox {
            frame: Frame::from_px(60.0, y_cursor, 200.0, 200.0),
            bytes: img.bytes,
            mime: img.mime,
            fit: ImageFit::Contain,
        }));
        y_cursor += 220.0;
    }

    // If we found nothing, drop in a placeholder shape so the slide
    // isn't completely empty (PowerPoint dislikes empty slides).
    if elements.is_empty() {
        elements.push(PptxElement::Shape(ShapeBox {
            frame: Frame::from_px(60.0, 60.0, 200.0, 200.0),
            shape: ShapeKind::Rect,
            fill: Fill::None,
            stroke: None,
            shadow: None,
        }));
    }

    PptxDeck {
        title: extract_title(html),
        slides: vec![PptxSlide {
            width_emu: px_to_emu(width_px as f32),
            height_emu: px_to_emu(height_px as f32),
            background,
            elements,
            speaker_notes: None,
        }],
    }
}

#[derive(Debug, Clone)]
struct TextBlock {
    text: String,
    size_pt: f32,
    weight: u32,
}

fn extract_text_blocks(html: &str) -> Vec<TextBlock> {
    let mut out: Vec<TextBlock> = Vec::new();
    push_tag(&mut out, html, "h1", 36.0, 700);
    push_tag(&mut out, html, "h2", 28.0, 600);
    push_tag(&mut out, html, "h3", 22.0, 600);
    push_tag(&mut out, html, "p", 14.0, 400);
    out
}

fn push_tag(out: &mut Vec<TextBlock>, html: &str, tag: &str, size_pt: f32, weight: u32) {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut start = 0usize;
    while let Some(open_idx) = lower[start..].find(&open) {
        let abs_open = start + open_idx;
        // Find the end of the opening tag.
        let after_open_tag = match html[abs_open..].find('>') {
            Some(n) => abs_open + n + 1,
            None => break,
        };
        let close_idx = match lower[after_open_tag..].find(&close) {
            Some(n) => after_open_tag + n,
            None => break,
        };
        let inner = &html[after_open_tag..close_idx];
        let text = strip_inline_tags(inner).trim().to_string();
        if !text.is_empty() {
            out.push(TextBlock {
                text,
                size_pt,
                weight,
            });
        }
        start = close_idx + close.len();
    }
}

fn strip_inline_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            continue;
        }
        if !in_tag {
            out.push(c);
        }
    }
    out
}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title>")?;
    let close = lower.find("</title>")?;
    if close <= open {
        return None;
    }
    Some(html[open + 7..close].trim().to_string())
}

fn extract_slide_size(html: &str) -> Option<(u32, u32)> {
    let w = extract_data_attr(html, "data-slide-w")?.parse().ok()?;
    let h = extract_data_attr(html, "data-slide-h")?.parse().ok()?;
    Some((w, h))
}

fn extract_data_attr(html: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let pos = html.find(&pattern)?;
    let start = pos + pattern.len();
    let end = html[start..].find('"')?;
    Some(html[start..start + end].to_string())
}

fn extract_body_background(html: &str) -> Option<Color> {
    let lower = html.to_ascii_lowercase();
    let body_open = lower.find("<body")?;
    let body_end = html[body_open..].find('>')?;
    let header = &html[body_open..body_open + body_end];
    // Look for `background:` or `background-color:` inside the style="".
    let style_pos = header.find("style=\"")?;
    let style_start = style_pos + 7;
    let style_end = header[style_start..].find('"')?;
    let style = &header[style_start..style_start + style_end];
    for prefix in ["background-color:", "background:"] {
        if let Some(p) = style.find(prefix) {
            let rest = &style[p + prefix.len()..];
            let v = rest.split(';').next().unwrap_or("").trim();
            if let Some(c) = crate::parse_css_color(v) {
                return Some(c);
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
struct DataUriImage {
    bytes: Vec<u8>,
    mime: ImageMime,
}

fn extract_data_uri_images(html: &str) -> Vec<DataUriImage> {
    use base64::Engine as _;
    let mut out: Vec<DataUriImage> = Vec::new();
    let mut start = 0usize;
    while let Some(idx) = html[start..].find("src=\"data:image/") {
        let abs = start + idx;
        // Find the end quote of the src attr.
        let body_start = abs + "src=\"".len();
        let end = match html[body_start..].find('"') {
            Some(n) => body_start + n,
            None => break,
        };
        let uri = &html[body_start..end];
        // uri = "data:image/<mime>;base64,<payload>"
        if let Some(rest) = uri.strip_prefix("data:") {
            if let Some((header, payload)) = rest.split_once(',') {
                let mime = if header.contains("png") {
                    Some(ImageMime::Png)
                } else if header.contains("jpeg") || header.contains("jpg") {
                    Some(ImageMime::Jpeg)
                } else {
                    None
                };
                if let (Some(mime), true) = (mime, header.contains(";base64")) {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) {
                        out.push(DataUriImage { bytes, mime });
                    }
                }
            }
        }
        start = end + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_html_yields_single_blank_slide() {
        let deck = from_html("");
        assert_eq!(deck.slides.len(), 1);
        // Default 1280×720.
        assert_eq!(deck.slides[0].width_emu, 12_192_000);
    }

    #[test]
    fn h1_and_p_each_become_text_box() {
        let html = "<body><h1>Hello</h1><p>World</p></body>";
        let deck = from_html(html);
        let elements = &deck.slides[0].elements;
        assert_eq!(elements.len(), 2);
        match &elements[0] {
            PptxElement::Text(tb) => {
                assert_eq!(tb.content, "Hello");
                assert_eq!(tb.style.font_weight, 700);
            }
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn body_background_color_extracted() {
        let html = r#"<body style="background:#ff0000;color:white"><p>x</p></body>"#;
        let deck = from_html(html);
        match deck.slides[0].background {
            Background::Solid { color } => {
                assert_eq!(color.to_hex6(), "FF0000");
            }
            _ => panic!("expected solid"),
        }
    }

    #[test]
    fn data_slide_attrs_override_default_size() {
        let html = r#"<body data-slide-w="1920" data-slide-h="1080"></body>"#;
        let deck = from_html(html);
        assert_eq!(deck.slides[0].width_emu, px_to_emu(1920.0));
        assert_eq!(deck.slides[0].height_emu, px_to_emu(1080.0));
    }

    #[test]
    fn title_extracted() {
        let html = "<html><head><title>My Deck</title></head></html>";
        let deck = from_html(html);
        assert_eq!(deck.title.as_deref(), Some("My Deck"));
    }

    #[test]
    fn empty_body_emits_placeholder_shape() {
        let deck = from_html("<body></body>");
        assert!(matches!(deck.slides[0].elements[0], PptxElement::Shape(_)));
    }

    #[test]
    fn data_uri_image_decoded() {
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkAAIAAAoAAv/lxKUAAAAASUVORK5CYII=";
        let html = format!("<body><h1>x</h1><img src=\"data:image/png;base64,{png_b64}\"></body>");
        let deck = from_html(&html);
        let elements = &deck.slides[0].elements;
        assert!(elements.iter().any(|e| matches!(e, PptxElement::Image(_))));
    }

    #[test]
    fn oklch_body_background_supported() {
        // 6c integration — extract_body_background pipes through parse_css_color.
        let html = r#"<body style="background:oklch(0.628 0.258 29.23)"><p>x</p></body>"#;
        let deck = from_html(html);
        match deck.slides[0].background {
            Background::Solid { color } => {
                // Approximate red.
                assert!(color.0 > 0xf0, "r byte {}", color.0);
            }
            _ => panic!("expected solid"),
        }
    }
}
