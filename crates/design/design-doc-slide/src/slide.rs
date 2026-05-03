use serde::{Deserialize, Serialize};

pub const CANVAS_WIDTH: u32 = 1280;
pub const CANVAS_HEIGHT: u32 = 720;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlideDoc {
    pub id: String,
    pub width_px: u32,
    pub height_px: u32,
    pub background: Background,
    pub elements: Vec<SlideElement>,
    /// Short navigator-chip label. Introduced in v2 for Deck view. `None`
    /// for legacy v1 JSON — serde defaults fill in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Speaker notes destined for PPTX notes slide (v2.1). Stored but not
    /// yet UI-exposed in v2.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_notes: Option<String>,
}

impl SlideDoc {
    pub fn empty(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            width_px: CANVAS_WIDTH,
            height_px: CANVAS_HEIGHT,
            background: Background::solid("#ffffff"),
            elements: Vec::new(),
            title: None,
            speaker_notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Background {
    Color { color: String },
    Gradient { from: String, to: String, angle_deg: f32 },
    Image { src: String },
}

impl Background {
    pub fn solid(color: impl Into<String>) -> Self {
        Self::Color { color: color.into() }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlideElement {
    Text {
        id: String,
        frame: Frame,
        content: String,
        style: TextStyle,
    },
    Shape {
        id: String,
        frame: Frame,
        shape: ShapeKind,
        fill: Fill,
        stroke: Option<Stroke>,
    },
    Image {
        id: String,
        frame: Frame,
        src: String,
        fit: ImageFit,
    },
}

impl SlideElement {
    pub fn id(&self) -> &str {
        match self {
            SlideElement::Text { id, .. }
            | SlideElement::Shape { id, .. }
            | SlideElement::Image { id, .. } => id,
        }
    }

    pub fn frame(&self) -> Frame {
        match self {
            SlideElement::Text { frame, .. }
            | SlideElement::Shape { frame, .. }
            | SlideElement::Image { frame, .. } => *frame,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextStyle {
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size_px")]
    pub font_size_px: f32,
    #[serde(default = "default_font_weight")]
    pub font_weight: u32,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    #[serde(default)]
    pub letter_spacing_px: f32,
    #[serde(default)]
    pub align: TextAlign,
}

// These helpers back per-field `#[serde(default = …)]` attributes on
// `TextStyle`. They exist because AI-emitted JSON (Gemini / Claude CLI) is
// occasionally lossy: the model will drop fields it considers "obvious" even
// though our few-shot prompt emits them. Before this, a single missing
// `line_height` would abort the entire generate response and surface as
// `missing field line_height at line 49` in the chat panel — observed in
// e2e verification on 2026-04-24. The Default impl below stays as-is so
// Rust-side construction still has a single source of truth.
fn default_font_family() -> String {
    "Pretendard".into()
}
fn default_font_size_px() -> f32 {
    24.0
}
fn default_font_weight() -> u32 {
    400
}
fn default_color() -> String {
    "#111111".into()
}
fn default_line_height() -> f32 {
    1.4
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: default_font_family(),
            font_size_px: default_font_size_px(),
            font_weight: default_font_weight(),
            color: default_color(),
            line_height: default_line_height(),
            letter_spacing_px: 0.0,
            align: TextAlign::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    RoundedRect,
    Circle,
    Line,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Fill {
    Solid { color: String },
    Gradient { from: String, to: String, angle_deg: f32 },
    None,
}

impl Fill {
    pub fn solid(color: impl Into<String>) -> Self {
        Self::Solid { color: color.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stroke {
    pub color: String,
    pub width_px: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Cover,
    Contain,
    Fill,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_text_element() -> SlideElement {
        SlideElement::Text {
            id: "title".into(),
            frame: Frame { x: 80.0, y: 80.0, w: 1120.0, h: 120.0 },
            content: "안녕, Canvas".into(),
            style: TextStyle {
                font_size_px: 64.0,
                font_weight: 700,
                ..Default::default()
            },
        }
    }

    #[test]
    fn empty_uses_canvas_defaults() {
        let doc = SlideDoc::empty("s1");
        assert_eq!(doc.width_px, 1280);
        assert_eq!(doc.height_px, 720);
        assert!(doc.elements.is_empty());
    }

    #[test]
    fn json_round_trip_preserves_structure() {
        let mut doc = SlideDoc::empty("slide-001");
        doc.elements.push(sample_text_element());
        doc.elements.push(SlideElement::Shape {
            id: "accent".into(),
            frame: Frame { x: 0.0, y: 700.0, w: 1280.0, h: 20.0 },
            shape: ShapeKind::Rect,
            fill: Fill::solid("#3b82f6"),
            stroke: None,
        });

        let json = serde_json::to_string(&doc).unwrap();
        let parsed: SlideDoc = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, doc);
    }

    #[test]
    fn element_id_accessor_works_for_all_variants() {
        let text = sample_text_element();
        assert_eq!(text.id(), "title");

        let shape = SlideElement::Shape {
            id: "bar".into(),
            frame: Frame { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
            shape: ShapeKind::Circle,
            fill: Fill::None,
            stroke: None,
        };
        assert_eq!(shape.id(), "bar");

        let image = SlideElement::Image {
            id: "logo".into(),
            frame: Frame { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
            src: "/img.png".into(),
            fit: ImageFit::Contain,
        };
        assert_eq!(image.id(), "logo");
    }

    #[test]
    fn background_gradient_serializes_with_tag() {
        let bg = Background::Gradient {
            from: "#000".into(),
            to: "#fff".into(),
            angle_deg: 45.0,
        };
        let json = serde_json::to_value(&bg).unwrap();
        assert_eq!(json["kind"], "gradient");
        assert_eq!(json["from"], "#000");
    }

    #[test]
    fn title_and_speaker_notes_round_trip() {
        // v2 adds navigator-label ("title") and PPTX speaker-notes slots to
        // SlideDoc. Both optional so legacy single-slide JSON keeps working.
        let mut doc = SlideDoc::empty("s6");
        doc.title = Some("표지".into());
        doc.speaker_notes = Some("이 슬라이드는 첫 화면입니다.".into());

        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["title"], "표지");
        assert_eq!(json["speaker_notes"], "이 슬라이드는 첫 화면입니다.");

        let parsed: SlideDoc = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("표지"));
        assert_eq!(parsed.speaker_notes.as_deref(), Some("이 슬라이드는 첫 화면입니다."));
    }

    #[test]
    fn text_style_tolerates_missing_fields_from_ai_output() {
        // Regression guard — e2e run on 2026-04-24 had Gemini emit a Text
        // element whose `style` object was missing `line_height`. Before the
        // per-field `#[serde(default = …)]` guard, serde would reject the
        // whole response with `missing field line_height at line 49`, and
        // the user would see an error bubble instead of their landing page.
        //
        // TextStyle must now fall back to its Default for any missing field
        // while preserving every field that IS provided.
        let partial = serde_json::json!({
            "font_family": "Inter",
            "font_size_px": 48.0,
            "font_weight": 700,
            "color": "#0ea5e9",
            // no line_height, letter_spacing_px, align
        });
        let parsed: TextStyle = serde_json::from_value(partial).unwrap();
        assert_eq!(parsed.font_family, "Inter");
        assert_eq!(parsed.font_size_px, 48.0);
        assert_eq!(parsed.font_weight, 700);
        assert_eq!(parsed.color, "#0ea5e9");
        assert_eq!(parsed.line_height, 1.4);
        assert_eq!(parsed.letter_spacing_px, 0.0);
        assert_eq!(parsed.align, TextAlign::Left);

        // Empty object is also legal — full fallback to Default.
        let empty: TextStyle = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(empty, TextStyle::default());
    }

    #[test]
    fn legacy_slide_doc_json_without_title_fields_still_parses() {
        // v1 on-disk JSON predates `title` and `speaker_notes`. Migration
        // must not force a re-write — defaults should fill the gap.
        let legacy = serde_json::json!({
            "id": "legacy-1",
            "width_px": 1280,
            "height_px": 720,
            "background": {"kind": "color", "color": "#fff"},
            "elements": [],
        });
        let parsed: SlideDoc = serde_json::from_value(legacy).unwrap();
        assert!(parsed.title.is_none(), "legacy JSON yields None title");
        assert!(parsed.speaker_notes.is_none(), "legacy JSON yields None speaker_notes");
    }

    #[test]
    fn background_solid_helper_produces_color_variant() {
        // `Background::solid` is the canonical constructor used by
        // `SlideDoc::empty` and most call sites — verify it produces the
        // tagged `Color` variant rather than e.g. an Image with a hex src.
        let bg = Background::solid("#ff00aa");
        assert_eq!(bg, Background::Color { color: "#ff00aa".into() });
        let json = serde_json::to_value(&bg).unwrap();
        assert_eq!(json["kind"], "color");
        assert_eq!(json["color"], "#ff00aa");
    }

    #[test]
    fn background_image_round_trips_with_kind_tag() {
        // Companion to `background_gradient_serializes_with_tag` — the Image
        // variant was added in v2.1 (notes + image data URI fix). Round-trip
        // ensures `kind: "image"` discriminator matches what the renderer
        // expects.
        let bg = Background::Image {
            src: "data:image/png;base64,iVBORw0KG…".into(),
        };
        let json = serde_json::to_value(&bg).unwrap();
        assert_eq!(json["kind"], "image");
        assert!(json["src"].as_str().unwrap().starts_with("data:image/png"));

        let parsed: Background = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, bg);
    }
}
