use serde::{Deserialize, Serialize};

use crate::color::{Color, Fill, Stroke};
use crate::geometry::{Emu, Frame};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PptxElement {
    Text(TextBox),
    Shape(ShapeBox),
    Image(ImageBox),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextBox {
    pub frame: Frame,
    /// Plain text. `\n` produces a hard line break in v1.
    pub content: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct TextStyle {
    pub font_family: String,
    pub font_size_pt: f32,
    pub font_weight: u32,   // 400 = normal, 700 = bold
    pub color: Color,
    pub align: TextAlign,
    pub line_height: f32,
    pub letter_spacing_pt: f32,

    /// `true` → `<a:rPr i="1"/>`. Independent of `font_weight`.
    pub italic: bool,

    /// Per-mille alpha (0..=100_000) matching OOXML `<a:alpha val=…/>`.
    /// `None` = fully opaque (no `<a:alpha>` tag emitted).
    pub color_alpha: Option<u32>,

    /// `true` (default) → `<a:bodyPr wrap="square">` (lines wrap inside frame).
    /// `false` → `<a:bodyPr wrap="none">` (text overflows frame, forcing a hard
    /// single line unless `\n` is present in `content`).
    pub allow_wrap: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Calibri".into(),
            font_size_pt: 18.0,
            font_weight: 400,
            color: Color(0x11, 0x11, 0x11),
            align: TextAlign::Left,
            line_height: 1.2,
            letter_spacing_pt: 0.0,
            italic: false,
            color_alpha: None,
            allow_wrap: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign { Left, Center, Right, Justify }

/// Outer drop shadow effect (CSS `box-shadow` equivalent for OOXML shapes).
///
/// Maps to `<a:effectLst><a:outerShdw …/>` in OOXML.
/// All offset/blur values are in CSS-style pixels (96 DPI); the writer converts
/// them to EMU internally (`1 px = 9525 EMU`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct OuterShadow {
    /// Horizontal offset in pixels. Positive = right.
    pub dx_px: f32,
    /// Vertical offset in pixels. Positive = down (matches OOXML convention).
    pub dy_px: f32,
    /// Blur radius in pixels. Clamped to ≥ 0 internally.
    pub blur_px: f32,
    /// Shadow color (sRGB).
    pub color: Color,
    /// Per-mille alpha (0..=100_000) matching OOXML `<a:alpha val=…/>`.
    /// `None` = fully opaque (no `<a:alpha>` tag emitted).
    pub alpha: Option<u32>,
}

impl OuterShadow {
    /// Build the `<a:effectLst><a:outerShdw …/>` XML fragment for this shadow.
    ///
    /// Used by both the write-only `writer::shape` path and the template-edit
    /// `template::xml_ops` path so the formula lives in one place.
    ///
    /// EMU conversion: `1 px = 9525 EMU`. Direction math: `dist = √(dx²+dy²) × 9525`,
    /// `dir = atan2(dy,dx) deg mod 360 × 60_000`. Ported from dear-jeongbin
    /// `build_outer_shadow_xml` (`export_pptx_ooxml.rs:1808-1841`).
    pub(crate) fn to_ooxml_effect_xml(&self) -> String {
        const EMU_PER_PX: f32 = 9525.0;
        let blur_rad = (self.blur_px.max(0.0) * EMU_PER_PX).round() as i64;
        let dist_px = (self.dx_px.powi(2) + self.dy_px.powi(2)).sqrt();
        let dist_emu = (dist_px * EMU_PER_PX).round() as i64;
        let dir_60k = if dist_px > 0.01 {
            let rad = self.dy_px.atan2(self.dx_px);
            let deg = rad.to_degrees();
            let norm = ((deg % 360.0) + 360.0) % 360.0;
            (norm * 60_000.0).round() as i64
        } else {
            0
        };
        let hex = self.color.to_hex6();
        let alpha_tag = match self.alpha {
            Some(a) if a < 100_000 => format!(r#"<a:alpha val="{a}"/>"#),
            _ => String::new(),
        };
        format!(
            concat!(
                r#"<a:effectLst>"#,
                r#"<a:outerShdw blurRad="{blur}" dist="{dist}" dir="{dir}" algn="tl" rotWithShape="0">"#,
                r#"<a:srgbClr val="{color}">{alpha}</a:srgbClr>"#,
                r#"</a:outerShdw>"#,
                r#"</a:effectLst>"#,
            ),
            blur = blur_rad,
            dist = dist_emu,
            dir = dir_60k,
            color = hex,
            alpha = alpha_tag,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct ShapeBox {
    pub frame: Frame,
    pub shape: ShapeKind,
    pub fill: Fill,
    pub stroke: Option<Stroke>,
    /// Optional outer shadow effect. `None` → no `<a:effectLst>` emitted.
    pub shadow: Option<OuterShadow>,
}

impl ShapeBox {
    /// Construct a `ShapeBox` without a shadow. Preferred over struct literals
    /// now that `ShapeBox` is `#[non_exhaustive]`.
    ///
    /// Equivalent to:
    /// ```rust
    /// # use design_export_pptx::{ShapeBox, ShapeKind, Fill, Color, Frame};
    /// # let frame = Frame::from_px(0.0, 0.0, 100.0, 50.0);
    /// # let shape = ShapeKind::Rect;
    /// # let fill = Fill::solid(Color::BLACK);
    /// let mut sb = ShapeBox::new(frame, shape, fill, None);
    /// // sb.shadow = Some(…);  // add shadow if needed
    /// ```
    pub fn new(frame: Frame, shape: ShapeKind, fill: Fill, stroke: Option<Stroke>) -> Self {
        Self { frame, shape, fill, stroke, shadow: None }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    RoundedRect { radius_emu: Emu },
    Ellipse,
    Line,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageBox {
    pub frame: Frame,
    pub bytes: Vec<u8>,
    pub mime: ImageMime,
    pub fit: ImageFit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageMime { Png, Jpeg }

impl ImageMime {
    pub fn ext(self) -> &'static str {
        match self { Self::Png => "png", Self::Jpeg => "jpg" }
    }
    pub fn content_type(self) -> &'static str {
        match self { Self::Png => "image/png", Self::Jpeg => "image/jpeg" }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageFit { Cover, Contain, Fill }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Frame;

    #[test]
    fn pptx_element_text_variant_round_trips_through_json() {
        let e = PptxElement::Text(TextBox {
            frame: Frame::from_px(0.0, 0.0, 100.0, 50.0),
            content: "hi".into(),
            style: TextStyle::default(),
        });
        let json = serde_json::to_string(&e).unwrap();
        let back: PptxElement = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn image_mime_content_types_match_ooxml_expectations() {
        assert_eq!(ImageMime::Png.content_type(), "image/png");
        assert_eq!(ImageMime::Jpeg.ext(), "jpg");
    }
}
