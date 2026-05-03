//! Bridge from [`kangnam_design_doc_slide`] types to the neutral PPTX IR.
//!
//! Gated behind the `slide-doc` Cargo feature (default-on). Write-only
//! consumers that assemble their own `PptxDeck` can disable the feature
//! to drop the `canvas-slide-doc` dependency entirely.

use kangnam_design_doc_slide::{
    Background as SdBackground, Deck, Fill as SdFill, ImageFit as SdFit, ShapeKind as SdShape,
    SlideDoc, SlideElement, TextAlign as SdAlign,
};
use thiserror::Error;

use crate as pw;

#[derive(Debug, Error)]
pub enum FromSlideDocError {
    #[error("invalid hex color '{0}'")]
    InvalidHex(String),
    #[error("image element references missing asset: {0}")]
    MissingImageAsset(String),
}

pub type Result<T> = std::result::Result<T, FromSlideDocError>;

/// Convert a [`Deck`] into a [`pw::PptxDeck`] ready for
/// [`pw::write_deck`].
pub fn from_deck(deck: &Deck) -> Result<pw::PptxDeck> {
    let slides: Result<Vec<_>> = deck.slides.iter().map(from_slide_doc).collect();
    Ok(pw::PptxDeck {
        title: Some(deck.id.clone()),
        slides: slides?,
    })
}

/// Convert a single [`SlideDoc`] into a [`pw::PptxSlide`].
pub fn from_slide_doc(doc: &SlideDoc) -> Result<pw::PptxSlide> {
    let mut elements: Vec<pw::PptxElement> =
        doc.elements.iter().map(convert_element).collect::<Result<_>>()?;
    let background = match &doc.background {
        SdBackground::Image { src } => {
            // Phase 6b-04: try to embed a data URI as a full-bleed
            // image at the front of the element list (renders behind
            // everything else), and use white as the writer-level bg.
            if let Some((mime, bytes)) = decode_data_uri(src) {
                let bleed = pw::ImageBox {
                    frame: pw::Frame::from_px(0.0, 0.0, doc.width_px as f32, doc.height_px as f32),
                    bytes,
                    mime,
                    fit: pw::ImageFit::Cover,
                };
                elements.insert(0, pw::PptxElement::Image(bleed));
            }
            pw::Background::Solid { color: pw::Color::WHITE }
        }
        other => convert_bg(other)?,
    };
    Ok(pw::PptxSlide {
        width_emu: pw::geometry::px_to_emu(doc.width_px as f32),
        height_emu: pw::geometry::px_to_emu(doc.height_px as f32),
        background,
        elements,
        speaker_notes: doc.speaker_notes.clone(),
    })
}

fn convert_element(e: &SlideElement) -> Result<pw::PptxElement> {
    match e {
        SlideElement::Text { frame, content, style, .. } => {
            Ok(pw::PptxElement::Text(pw::TextBox {
                frame: pw::Frame::from_px(frame.x, frame.y, frame.w, frame.h),
                content: content.clone(),
                style: pw::TextStyle {
                    font_family: style.font_family.clone(),
                    font_size_pt: style.font_size_px * 0.75, // 96dpi: 1px = 0.75pt
                    font_weight: style.font_weight,
                    color: parse_hex(&style.color)?,
                    align: match style.align {
                        SdAlign::Left => pw::TextAlign::Left,
                        SdAlign::Center => pw::TextAlign::Center,
                        SdAlign::Right => pw::TextAlign::Right,
                        SdAlign::Justify => pw::TextAlign::Justify,
                    },
                    line_height: style.line_height,
                    letter_spacing_pt: style.letter_spacing_px * 0.75,
                    ..Default::default()
                },
            }))
        }
        SlideElement::Shape { frame, shape, fill, stroke, .. } => {
            let kind = match shape {
                SdShape::Rect => pw::ShapeKind::Rect,
                SdShape::RoundedRect => pw::ShapeKind::RoundedRect {
                    radius_emu: pw::geometry::px_to_emu(12.0), // default corner radius
                },
                SdShape::Circle => pw::ShapeKind::Ellipse,
                SdShape::Line => pw::ShapeKind::Line,
            };
            let pw_fill = match fill {
                SdFill::Solid { color } => pw::Fill::solid(parse_hex(color)?),
                #[allow(deprecated)]
                SdFill::Gradient { from, to, angle_deg } => pw::Fill::Gradient {
                    from: parse_hex(from)?,
                    to: parse_hex(to)?,
                    angle_deg: *angle_deg,
                },
                SdFill::None => pw::Fill::None,
            };
            let pw_stroke = stroke
                .as_ref()
                .map(|s| {
                    Ok::<_, FromSlideDocError>(pw::Stroke {
                        color: parse_hex(&s.color)?,
                        width_emu: pw::geometry::px_to_emu(s.width_px),
                    })
                })
                .transpose()?;
            Ok(pw::PptxElement::Shape(pw::ShapeBox {
                frame: pw::Frame::from_px(frame.x, frame.y, frame.w, frame.h),
                shape: kind,
                fill: pw_fill,
                stroke: pw_stroke,
                shadow: None,
            }))
        }
        SlideElement::Image { frame, src, fit, .. } => {
            // Phase 6b-03: try to decode a `data:image/...;base64,...`
            // URI so the image actually embeds in the PPTX. Other URL
            // schemes (https, file) still fall back to a transparent
            // rect since the writer doesn't fetch.
            let pw_fit = match fit {
                SdFit::Cover => pw::ImageFit::Cover,
                SdFit::Contain => pw::ImageFit::Contain,
                SdFit::Fill => pw::ImageFit::Fill,
            };
            if let Some((mime, bytes)) = decode_data_uri(src) {
                Ok(pw::PptxElement::Image(pw::ImageBox {
                    frame: pw::Frame::from_px(frame.x, frame.y, frame.w, frame.h),
                    bytes,
                    mime,
                    fit: pw_fit,
                }))
            } else {
                Ok(pw::PptxElement::Shape(pw::ShapeBox {
                    frame: pw::Frame::from_px(frame.x, frame.y, frame.w, frame.h),
                    shape: pw::ShapeKind::Rect,
                    fill: pw::Fill::None,
                    stroke: None,
                    shadow: None,
                }))
            }
        }
    }
}

fn convert_bg(bg: &SdBackground) -> Result<pw::Background> {
    match bg {
        SdBackground::Color { color } => Ok(pw::Background::Solid {
            color: parse_hex(color)?,
        }),
        SdBackground::Gradient { from, to, angle_deg } => Ok(pw::Background::Gradient {
            from: parse_hex(from)?,
            to: parse_hex(to)?,
            angle_deg: *angle_deg,
        }),
        SdBackground::Image { .. } => {
            // Not supported in v1 — fall back to white.
            Ok(pw::Background::Solid {
                color: pw::Color::WHITE,
            })
        }
    }
}

/// Parse a CSS hex color string into an RGB [`pw::Color`].
///
/// Accepts `#RRGGBB`, `#RRGGBBAA` (alpha stripped — PPTX `srgbClr` is
/// RGB-only), and the bare forms without `#`. 3-char shorthand (`#RGB`)
/// is expanded.
/// Decode a `data:image/<png|jpeg>;base64,<...>` URI into raw bytes +
/// MIME enum. Returns None for non-data URIs, unsupported MIME types,
/// or malformed base64.
fn decode_data_uri(src: &str) -> Option<(pw::ImageMime, Vec<u8>)> {
    use base64::Engine as _;
    let rest = src.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let mime = if header.contains("image/png") {
        pw::ImageMime::Png
    } else if header.contains("image/jpeg") || header.contains("image/jpg") {
        pw::ImageMime::Jpeg
    } else {
        return None;
    };
    if !header.contains(";base64") {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    Some((mime, bytes))
}

/// Parse a CSS color string into an RGB [`pw::Color`].
///
/// Phase 6c-01 widened the accepted set beyond hex:
/// - `#RRGGBB`, `#RRGGBBAA` (alpha stripped), `#RGB` shorthand
/// - `oklch(L C H)` / `oklch(L C H / A)` (alpha stripped)
/// - `oklab(L a b)` / `oklab(L a b / A)`
/// - `color(display-p3 R G B)` / `color(srgb R G B)`
/// - `rgb(R, G, B)` / `rgba(R, G, B, A)`
///
/// All conversions reduce to PPTX-friendly sRGB (PowerPoint `srgbClr`
/// is sRGB-only). The OKLch ground-truth design tokens that come out
/// of design-system catalog scans round-trip through this path.
pub(crate) fn parse_hex(s: &str) -> Result<pw::Color> {
    pw::parse_css_color(s).ok_or_else(|| FromSlideDocError::InvalidHex(s.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kangnam_design_doc_slide::{Background, Frame as SdFrame, SlideDoc, SlideElement, TextStyle};

    #[test]
    fn text_element_converts_px_to_pt_correctly() {
        let mut doc = SlideDoc::empty("s1");
        doc.elements.push(SlideElement::Text {
            id: "t".into(),
            frame: SdFrame { x: 0.0, y: 0.0, w: 100.0, h: 50.0 },
            content: "hello".into(),
            style: TextStyle { font_size_px: 24.0, font_weight: 700, ..Default::default() },
        });
        let pptx = from_slide_doc(&doc).unwrap();
        match &pptx.elements[0] {
            pw::PptxElement::Text(tb) => {
                assert_eq!(tb.style.font_size_pt, 18.0); // 24 * 0.75
                assert_eq!(tb.style.font_weight, 700);
                assert_eq!(tb.content, "hello");
            }
            _ => panic!("expected text element"),
        }
    }

    #[test]
    fn invalid_hex_in_color_is_explicit_error() {
        let mut doc = SlideDoc::empty("s1");
        doc.background = Background::Color { color: "not-a-hex".into() };
        assert!(matches!(
            from_slide_doc(&doc).unwrap_err(),
            FromSlideDocError::InvalidHex(_)
        ));
    }

    #[test]
    fn eight_char_rrggbbaa_strips_alpha() {
        // #38BDF822 is the real color Gemini CLI produced that broke
        // PPTX export. Adapter must strip the alpha byte and parse the
        // RGB prefix.
        let mut doc = SlideDoc::empty("s1");
        doc.background = Background::Color { color: "#38BDF822".into() };
        let p = from_slide_doc(&doc).unwrap();
        match p.background {
            pw::Background::Solid { color } => {
                assert_eq!(color.to_hex6(), "38BDF8");
            }
            _ => panic!("expected solid background"),
        }
    }

    #[test]
    fn three_char_shorthand_expands() {
        let mut doc = SlideDoc::empty("s1");
        doc.background = Background::Color { color: "#F00".into() };
        let p = from_slide_doc(&doc).unwrap();
        match p.background {
            pw::Background::Solid { color } => {
                assert_eq!(color.to_hex6(), "FF0000");
            }
            _ => panic!("expected solid background"),
        }
    }

    #[test]
    fn gradient_background_passes_through() {
        let mut doc = SlideDoc::empty("s1");
        doc.background = Background::Gradient {
            from: "#FF0000".into(),
            to: "#0000FF".into(),
            angle_deg: 45.0,
        };
        let p = from_slide_doc(&doc).unwrap();
        match p.background {
            pw::Background::Gradient { angle_deg, .. } => assert_eq!(angle_deg, 45.0),
            _ => panic!(),
        }
    }
}
