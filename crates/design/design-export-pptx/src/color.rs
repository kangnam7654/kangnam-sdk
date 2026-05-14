use std::io::Cursor;

use image::{ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};

use crate::error::{PptxWriteError, Result};

/// RGB color in sRGB space, 0..=255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub const WHITE: Color = Color(255, 255, 255);

    /// Format as 6-char uppercase hex, no leading `#`. OOXML's `<a:srgbClr val="…">`
    /// wants this exact form.
    pub fn to_hex6(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    /// Parse a #RRGGBB or RRGGBB string. `None` on malformed input.
    pub fn from_hex(s: &str) -> Option<Color> {
        let s = s.strip_prefix('#').unwrap_or(s);
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Color(r, g, b))
    }
}

/// A single stop in a multi-stop gradient.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GradientStop {
    /// Position along the gradient axis, 0.0..=1.0. The writer does NOT
    /// re-sort stops; callers are responsible for ascending order.
    ///
    /// In debug builds, `to_ooxml_fill_xml` asserts stops are ordered.
    pub position: f32,
    pub color: Color,
    /// Per-mille alpha (0..=100_000, where 0 = transparent, 100_000 = opaque).
    /// `None` = fully opaque.
    pub alpha: Option<u32>,
}

/// Fill variant for a shape or slide background.
///
/// # Breaking change in v0.3.2
/// `Fill::Solid` gained an `alpha` field. Use [`Fill::solid`] to construct a
/// fully-opaque solid fill without specifying `alpha: None` explicitly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Fill {
    /// Single solid color.
    ///
    /// `alpha` is per-mille (0..=100_000); `None` = fully opaque. Use
    /// [`Fill::solid`] to construct without specifying `alpha`.
    Solid { color: Color, alpha: Option<u32> },

    /// Multi-stop linear gradient. `angle_deg` is CSS convention
    /// (0° = up, clockwise); converted to OOXML angle internally.
    LinearGradient {
        angle_deg: f32,
        stops: Vec<GradientStop>,
    },

    /// Multi-stop radial gradient (`<a:path path="circle"/>` centered at
    /// 50%/50%). v1: center is fixed; no ellipse mode.
    RadialGradient { stops: Vec<GradientStop> },

    /// 1:1 PNG tile fill, top-left aligned. The PNG is embedded as
    /// `ppt/media/imageN.png`. Use [`Fill::dot_tile`] to generate a
    /// common anti-aliased dot tile PNG.
    TilePattern {
        png_bytes: Vec<u8>,
        tile_w_px: u32,
        tile_h_px: u32,
    },

    /// 2-stop linear gradient — **deprecated** alias of `LinearGradient`
    /// with stops at 0% (`from`) and 100% (`to`).
    ///
    /// Kept for v0.2/v0.3.0 consumers; new code should use `LinearGradient`.
    /// Will be removed in v0.4.0.
    #[deprecated(
        since = "0.3.2",
        note = "use Fill::LinearGradient { angle_deg, stops } instead"
    )]
    Gradient {
        from: Color,
        to: Color,
        angle_deg: f32,
    },

    /// No fill — `<a:noFill/>`.
    None,
}

impl Fill {
    /// Construct a fully-opaque solid fill.
    ///
    /// Preferred over `Fill::Solid { color, alpha: None }` to avoid needing to
    /// specify the `alpha` field added in v0.3.2.
    pub fn solid(color: Color) -> Self {
        Fill::Solid { color, alpha: None }
    }

    /// Build a `Fill::TilePattern` whose PNG is a single anti-aliased dot
    /// centered in a `tile_w_px × tile_h_px` transparent canvas.
    ///
    /// Uses 2× supersampling + 2×2 box downsample to reduce LibreOffice's
    /// bilinear-scaling brightening of the AA band.
    ///
    /// # Errors
    /// Returns `Err` if `tile_w_px == 0` or `tile_h_px == 0`.
    pub fn dot_tile(
        tile_w_px: u32,
        tile_h_px: u32,
        dot_radius_px: f32,
        color_rgba: [u8; 4],
    ) -> Result<Self> {
        if tile_w_px == 0 || tile_h_px == 0 {
            return Err(PptxWriteError::Xml(
                "dot_tile: tile_w_px and tile_h_px must be > 0".into(),
            ));
        }
        let png_bytes = build_dot_tile_png(tile_w_px, tile_h_px, dot_radius_px, color_rgba)?;
        Ok(Fill::TilePattern {
            png_bytes,
            tile_w_px,
            tile_h_px,
        })
    }

    /// Emit the OOXML fill XML fragment for this variant.
    ///
    /// Stable public API as of v0.3.4. Useful for ad-hoc XML composition (e.g.
    /// in `dom-to-pptx-translator` or other pipelines that build `<p:sp>` XML
    /// manually). For the standard shape-injection path, prefer
    /// [`crate::template::PptxTemplate::add_element`] with
    /// `PptxElement::Shape(ShapeBox { fill, .. })`.
    ///
    /// # Panics / empty string for `TilePattern`
    /// `TilePattern` requires embedding a PNG into the zip and mutating slide
    /// rels — neither of which is possible from a `&self` context. This method
    /// returns an **empty string** for that variant. Use
    /// [`crate::template::PptxTemplate::add_element`] with
    /// `Fill::TilePattern` for tile fills (available since v0.3.4).
    pub fn to_ooxml_fill_xml(&self) -> String {
        #[allow(deprecated)]
        match self {
            Fill::Solid { color, alpha } => solid_fill_xml(color, *alpha),
            Fill::LinearGradient { angle_deg, stops } => linear_gradient_xml(*angle_deg, stops),
            Fill::RadialGradient { stops } => radial_gradient_xml(stops),
            Fill::TilePattern { .. } => {
                // Cannot emit: requires PNG embed into zip + slide rels mutation.
                // Use add_element(PptxElement::Shape(ShapeBox { fill: Fill::TilePattern{..} }))
                // (available since v0.3.4) for tile fills.
                String::new()
            }
            Fill::Gradient {
                from,
                to,
                angle_deg,
            } => {
                let stops = vec![
                    GradientStop {
                        position: 0.0,
                        color: *from,
                        alpha: None,
                    },
                    GradientStop {
                        position: 1.0,
                        color: *to,
                        alpha: None,
                    },
                ];
                linear_gradient_xml(*angle_deg, &stops)
            }
            Fill::None => "<a:noFill/>".to_string(),
        }
    }
}

// ── OOXML emission helpers (pub(crate) for writer/shape.rs) ───────────────

/// `<a:solidFill>…</a:solidFill>` fragment.
pub(crate) fn solid_fill_xml(color: &Color, alpha: Option<u32>) -> String {
    let hex = color.to_hex6();
    match alpha {
        Some(a) if a < 100_000 => format!(
            r#"<a:solidFill><a:srgbClr val="{hex}"><a:alpha val="{a}"/></a:srgbClr></a:solidFill>"#,
        ),
        _ => format!(r#"<a:solidFill><a:srgbClr val="{hex}"/></a:solidFill>"#),
    }
}

/// `<a:gsLst>…</a:gsLst>` fragment shared by linear and radial gradients.
pub(crate) fn gs_list_xml(stops: &[GradientStop]) -> String {
    debug_assert!(
        stops.windows(2).all(|w| w[0].position <= w[1].position),
        "gradient stops must be in ascending position order; got {:?}",
        stops.iter().map(|s| s.position).collect::<Vec<_>>()
    );
    let mut s = String::from("<a:gsLst>");
    for stop in stops {
        let p = (stop.position.clamp(0.0, 1.0) * 100_000.0).round() as i64;
        let hex = stop.color.to_hex6();
        let inner = match stop.alpha {
            Some(a) if a < 100_000 => {
                format!(r#"<a:srgbClr val="{hex}"><a:alpha val="{a}"/></a:srgbClr>"#,)
            }
            _ => format!(r#"<a:srgbClr val="{hex}"/>"#),
        };
        s.push_str(&format!(r#"<a:gs pos="{p}">{inner}</a:gs>"#));
    }
    s.push_str("</a:gsLst>");
    s
}

/// Convert CSS gradient angle (0° = up, clockwise) to OOXML 1/60000-degree
/// units (0° = right, clockwise). Formula: `ooxml_deg = (css_deg − 90) mod 360`.
pub(crate) fn css_to_ooxml_angle(css_deg: f32) -> i64 {
    let mut deg = (css_deg - 90.0) % 360.0;
    if deg < 0.0 {
        deg += 360.0;
    }
    (deg * 60_000.0).round() as i64
}

/// `<a:gradFill … ><a:gsLst>…</a:gsLst><a:lin …/></a:gradFill>` fragment.
pub(crate) fn linear_gradient_xml(angle_deg: f32, stops: &[GradientStop]) -> String {
    let gs = gs_list_xml(stops);
    let ang = css_to_ooxml_angle(angle_deg);
    format!(
        r#"<a:gradFill flip="none" rotWithShape="1">{gs}<a:lin ang="{ang}" scaled="0"/></a:gradFill>"#,
    )
}

/// `<a:gradFill …><a:gsLst>…</a:gsLst><a:path path="circle">…</a:path></a:gradFill>` fragment.
pub(crate) fn radial_gradient_xml(stops: &[GradientStop]) -> String {
    let gs = gs_list_xml(stops);
    format!(
        r#"<a:gradFill flip="none" rotWithShape="1">{gs}<a:path path="circle"><a:fillToRect l="50000" t="50000" r="50000" b="50000"/></a:path></a:gradFill>"#,
    )
}

/// `<a:blipFill …>…<a:tile …/></a:blipFill>` fragment.
/// `rel_id` is the `rId` string for the embedded PNG in the slide rels.
pub(crate) fn blip_tile_fill_xml(rel_id: &str) -> String {
    format!(
        concat!(
            r#"<a:blipFill rotWithShape="1">"#,
            r#"<a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="{rid}"/>"#,
            r#"<a:srcRect/>"#,
            r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="tl"/>"#,
            r#"</a:blipFill>"#,
        ),
        rid = rel_id,
    )
}

// ── dot tile PNG generator ────────────────────────────────────────────────

fn build_dot_tile_png(
    tile_w_px: u32,
    tile_h_px: u32,
    dot_radius_px: f32,
    color_rgba: [u8; 4],
) -> Result<Vec<u8>> {
    // 2× supersampling + 2×2 box downsample. Reduces LibreOffice bilinear-scaling
    // brightening of the AA band (ported from dear-jeongbin export_pptx_ooxml.rs).
    const SS: u32 = 2;
    let w_hi = tile_w_px * SS;
    let h_hi = tile_h_px * SS;
    let mut hi = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(w_hi, h_hi, Rgba([0, 0, 0, 0]));

    let cx = w_hi as f32 / 2.0;
    let cy = h_hi as f32 / 2.0;
    let r_hi = (dot_radius_px * SS as f32).max(0.5);
    let outer = r_hi + 0.5;
    let inner = (r_hi - 0.5).max(0.0);
    let base_alpha = color_rgba[3] as f32;

    for y in 0..h_hi {
        for x in 0..w_hi {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let coverage = if d <= inner {
                1.0_f32
            } else if d >= outer {
                0.0_f32
            } else {
                outer - d
            };
            if coverage > 0.0 {
                let a = (base_alpha * coverage).round().clamp(0.0, 255.0) as u8;
                hi.put_pixel(x, y, Rgba([color_rgba[0], color_rgba[1], color_rgba[2], a]));
            }
        }
    }

    // 2×2 box downsample
    let mut img =
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(tile_w_px, tile_h_px, Rgba([0, 0, 0, 0]));
    for y in 0..tile_h_px {
        for x in 0..tile_w_px {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;
            for dy in 0..SS {
                for dx in 0..SS {
                    let p = hi.get_pixel(x * SS + dx, y * SS + dy).0;
                    r_sum += p[0] as u32;
                    g_sum += p[1] as u32;
                    b_sum += p[2] as u32;
                    a_sum += p[3] as u32;
                }
            }
            let n = SS * SS;
            img.put_pixel(
                x,
                y,
                Rgba([
                    (r_sum / n) as u8,
                    (g_sum / n) as u8,
                    (b_sum / n) as u8,
                    (a_sum / n) as u8,
                ]),
            );
        }
    }

    let mut buf = Cursor::new(Vec::<u8>::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| PptxWriteError::Xml(format!("dot tile PNG encode failed: {e}")))?;
    Ok(buf.into_inner())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Background {
    Solid {
        color: Color,
    },
    Gradient {
        from: Color,
        to: Color,
        angle_deg: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width_emu: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_uppercase() {
        let c = Color(0x3B, 0x82, 0xF6);
        assert_eq!(c.to_hex6(), "3B82F6");
        assert_eq!(Color::from_hex("#3B82F6"), Some(c));
        assert_eq!(Color::from_hex("3b82f6"), Some(c)); // lower tolerated
    }

    #[test]
    fn hex_rejects_bad_input() {
        assert_eq!(Color::from_hex("nope"), None);
        assert_eq!(Color::from_hex("#FF"), None);
        assert_eq!(Color::from_hex("#GGGGGG"), None);
    }

    #[test]
    fn fill_solid_serializes_with_kind_tag() {
        let f = Fill::solid(Color::BLACK);
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!(v["kind"], "solid");
        assert_eq!(v["color"], serde_json::json!([0, 0, 0]));
    }

    // ── unit tests for new API ────────────────────────────────────────────

    #[test]
    fn fill_solid_constructor_defaults_alpha_none() {
        let f = Fill::solid(Color::WHITE);
        assert_eq!(
            f,
            Fill::Solid {
                color: Color::WHITE,
                alpha: None
            }
        );
    }

    #[test]
    fn fill_solid_with_alpha_round_trips_serde() {
        let f = Fill::Solid {
            color: Color(255, 0, 0),
            alpha: Some(50_000),
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Fill = serde_json::from_str(&json).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn gradient_stop_serde_round_trip() {
        let stop = GradientStop {
            position: 0.5,
            color: Color(100, 200, 50),
            alpha: Some(30_000),
        };
        let json = serde_json::to_string(&stop).unwrap();
        let back: GradientStop = serde_json::from_str(&json).unwrap();
        assert_eq!(stop, back);
    }

    #[test]
    fn to_ooxml_fill_xml_solid_emits_srgb_clr() {
        let f = Fill::solid(Color(0x3B, 0x82, 0xF6));
        let xml = f.to_ooxml_fill_xml();
        assert!(xml.contains("<a:solidFill>"), "has solidFill: {xml}");
        assert!(xml.contains(r#"val="3B82F6""#), "has hex val: {xml}");
        assert!(!xml.contains("<a:alpha"), "no alpha tag for opaque: {xml}");
    }

    #[test]
    fn to_ooxml_fill_xml_solid_with_alpha_emits_alpha_tag() {
        let f = Fill::Solid {
            color: Color(0xFF, 0xFF, 0xFF),
            alpha: Some(50_000),
        };
        let xml = f.to_ooxml_fill_xml();
        assert!(xml.contains("<a:solidFill>"), "has solidFill: {xml}");
        assert!(
            xml.contains(r#"<a:alpha val="50000"/>"#),
            "has alpha tag: {xml}"
        );
    }

    #[test]
    fn css_to_ooxml_angle_conversion() {
        // CSS 0° (up) → OOXML 270° → 270 * 60_000 = 16_200_000
        assert_eq!(css_to_ooxml_angle(0.0), 16_200_000);
        // CSS 90° → OOXML 0° → 0
        assert_eq!(css_to_ooxml_angle(90.0), 0);
        // CSS 180° → OOXML 90° → 5_400_000
        assert_eq!(css_to_ooxml_angle(180.0), 5_400_000);
        // CSS 270° → OOXML 180° → 10_800_000
        assert_eq!(css_to_ooxml_angle(270.0), 10_800_000);
    }

    #[test]
    fn to_ooxml_fill_xml_no_fill() {
        let xml = Fill::None.to_ooxml_fill_xml();
        assert_eq!(xml, "<a:noFill/>");
    }
}
