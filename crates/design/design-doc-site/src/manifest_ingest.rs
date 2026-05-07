//! DOM walker manifest → [`SlideDoc`].
//!
//! The browser-side walker (added in W4) produces a JSON manifest with shapes in
//! z-order. This module converts that manifest into the canonical `SlideDoc` so
//! the same value can be stored, re-rendered, or re-exported via dom_to_pptx.
//!
//! W1 covers the minimum subset: `text` and `rect` shapes with solid fills.
//! Gradient / image / dot-pattern fills are deferred to W4 alongside the
//! complete dom_to_pptx port.

use kangnam_design_doc_slide::slide::{
    Background, Fill, Frame, ShapeKind, SlideDoc, SlideElement, Stroke, TextAlign, TextStyle,
    CANVAS_HEIGHT, CANVAS_WIDTH,
};

pub use crate::model::{
    Manifest, ManifestColor, ManifestError, ManifestRect, ManifestShape, ManifestText,
};

pub fn slide_from_manifest(m: Manifest) -> Result<SlideDoc, ManifestError> {
    let width_px = m.width_px.unwrap_or(CANVAS_WIDTH);
    let height_px = m.height_px.unwrap_or(CANVAS_HEIGHT);

    let background = match m.background {
        Some(c) => Background::solid(c.to_css()),
        None => Background::solid("#ffffff"),
    };

    let mut elements = Vec::with_capacity(m.shapes.len());
    for shape in m.shapes {
        elements.push(convert_shape(shape)?);
    }

    Ok(SlideDoc {
        id: m.slide_id,
        width_px,
        height_px,
        background,
        elements,
        title: None,
        speaker_notes: None,
    })
}

fn convert_shape(shape: ManifestShape) -> Result<SlideElement, ManifestError> {
    match shape {
        ManifestShape::Text(t) => Ok(SlideElement::Text {
            id: t.id,
            frame: Frame { x: t.x, y: t.y, w: t.w, h: t.h },
            content: t.content,
            style: TextStyle {
                font_family: "Pretendard".into(),
                font_size_px: t.font_size_px,
                font_weight: t.font_weight,
                color: t.color.map(|c| c.to_css()).unwrap_or_else(|| "#111111".into()),
                line_height: 1.4,
                letter_spacing_px: 0.0,
                align: parse_align(t.align.as_deref()),
            },
        }),
        ManifestShape::Rect(r) => Ok(SlideElement::Shape {
            id: r.id,
            frame: Frame { x: r.x, y: r.y, w: r.w, h: r.h },
            shape: if r.border_radius_px > 0.0 {
                ShapeKind::RoundedRect
            } else {
                ShapeKind::Rect
            },
            fill: match r.fill {
                Some(c) => Fill::solid(c.to_css()),
                None => Fill::None,
            },
            stroke: match (r.stroke_color, r.stroke_width_px) {
                (Some(c), Some(w)) if w > 0.0 => Some(Stroke {
                    color: c.to_css(),
                    width_px: w,
                }),
                _ => None,
            },
        }),
    }
}

fn parse_align(s: Option<&str>) -> TextAlign {
    match s.unwrap_or("left") {
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        "justify" => TextAlign::Justify,
        _ => TextAlign::Left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json() -> &'static str {
        r#"{
            "slide_id": "demo",
            "width_px": 1280,
            "height_px": 720,
            "background": { "r": 248, "g": 250, "b": 252 },
            "shapes": [
                {
                    "kind": "text",
                    "id": "title",
                    "x": 80, "y": 80, "w": 1120, "h": 120,
                    "content": "Canvas",
                    "color": { "r": 17, "g": 17, "b": 17 },
                    "font_size_px": 64,
                    "font_weight": 700,
                    "align": "center"
                },
                {
                    "kind": "rect",
                    "id": "accent",
                    "x": 0, "y": 700, "w": 1280, "h": 20,
                    "fill": { "r": 59, "g": 130, "b": 246 }
                }
            ]
        }"#
    }

    #[test]
    fn converts_text_and_rect() {
        let m: Manifest = serde_json::from_str(manifest_json()).unwrap();
        let doc = slide_from_manifest(m).unwrap();

        assert_eq!(doc.id, "demo");
        assert_eq!(doc.width_px, 1280);
        assert_eq!(doc.elements.len(), 2);

        match &doc.elements[0] {
            SlideElement::Text { id, content, style, .. } => {
                assert_eq!(id, "title");
                assert_eq!(content, "Canvas");
                assert_eq!(style.font_size_px, 64.0);
                assert_eq!(style.align, TextAlign::Center);
            }
            _ => panic!("expected text"),
        }

        match &doc.elements[1] {
            SlideElement::Shape { fill, shape, .. } => {
                assert!(matches!(shape, ShapeKind::Rect));
                assert!(matches!(fill, Fill::Solid { color } if color == "#3b82f6"));
            }
            _ => panic!("expected shape"),
        }
    }

    #[test]
    fn missing_dimensions_fall_back_to_canvas_defaults() {
        let m: Manifest = serde_json::from_str(
            r#"{"slide_id":"x","shapes":[]}"#,
        )
        .unwrap();
        let doc = slide_from_manifest(m).unwrap();
        assert_eq!(doc.width_px, CANVAS_WIDTH);
        assert_eq!(doc.height_px, CANVAS_HEIGHT);
    }

    #[test]
    fn rounded_rect_inferred_from_border_radius() {
        let json = r#"{
            "slide_id": "r",
            "shapes": [
                { "kind": "rect", "id": "card", "x": 0, "y": 0, "w": 100, "h": 100,
                  "fill": { "r": 0, "g": 0, "b": 0 }, "border_radius_px": 12 }
            ]
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let doc = slide_from_manifest(m).unwrap();
        match &doc.elements[0] {
            SlideElement::Shape { shape, .. } => {
                assert!(matches!(shape, ShapeKind::RoundedRect));
            }
            _ => panic!("expected shape"),
        }
    }

    #[test]
    fn rgba_alpha_is_preserved() {
        let json = r#"{
            "slide_id": "a",
            "shapes": [
                { "kind": "rect", "id": "tint", "x": 0, "y": 0, "w": 10, "h": 10,
                  "fill": { "r": 255, "g": 0, "b": 0, "a": 128 } }
            ]
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let doc = slide_from_manifest(m).unwrap();
        match &doc.elements[0] {
            SlideElement::Shape { fill: Fill::Solid { color }, .. } => {
                assert!(color.starts_with("rgba("), "got: {color}");
            }
            _ => panic!("expected solid fill"),
        }
    }
}
