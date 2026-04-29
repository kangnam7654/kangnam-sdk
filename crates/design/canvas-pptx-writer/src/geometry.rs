use serde::{Deserialize, Serialize};

/// EMU = English Metric Unit. 914_400 per inch. OOXML's native unit.
pub type Emu = i64;

/// 1 pixel at 96 dpi.
pub const EMU_PER_PX: Emu = 9_525;
/// 1 point = 1/72 inch.
pub const EMU_PER_PT: Emu = 12_700;

/// Slide-relative rectangle in EMU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub x_emu: Emu,
    pub y_emu: Emu,
    pub w_emu: Emu,
    pub h_emu: Emu,
}

impl Frame {
    pub fn from_px(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x_emu: (x * EMU_PER_PX as f32) as Emu,
            y_emu: (y * EMU_PER_PX as f32) as Emu,
            w_emu: (w * EMU_PER_PX as f32) as Emu,
            h_emu: (h * EMU_PER_PX as f32) as Emu,
        }
    }
}

pub fn px_to_emu(px: f32) -> Emu {
    (px * EMU_PER_PX as f32) as Emu
}

pub fn pt_to_emu(pt: f32) -> Emu {
    (pt * EMU_PER_PT as f32) as Emu
}

/// PPTX text sizes are "hundredths of points" inside `<a:rPr sz="…">`.
pub fn pt_to_hundredths(pt: f32) -> i64 {
    (pt * 100.0) as i64
}

/// Encode a `RoundedRect` corner radius as the OOXML `roundRect` `adj` value.
///
/// OOXML's `<a:gd name="adj" fmla="val N"/>` for `roundRect` expresses the
/// corner radius as a fraction of the shape's smaller dimension in
/// 1/100_000ths, capped at 50% (= 50_000). Used by both the write-only
/// `writer::shape` path and the template-edit `template::xml_ops` path so
/// the formula is canonical and lives in one place.
pub fn roundrect_adj(radius_emu: Emu, w_emu: Emu, h_emu: Emu) -> i64 {
    let min_dim = w_emu.min(h_emu).max(1);
    ((radius_emu.saturating_mul(100_000)) / min_dim).clamp(0, 50_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_matches_ooxml_reference() {
        // 1280 px slide width at 96 dpi = 12_192_000 EMU, the exact value
        // PowerPoint emits for a 13.333" (16:9 widescreen) deck.
        assert_eq!(px_to_emu(1280.0), 12_192_000);
        assert_eq!(px_to_emu(720.0), 6_858_000);
    }

    #[test]
    fn pt_to_hundredths_is_sz_value() {
        // 24 pt → sz="2400"
        assert_eq!(pt_to_hundredths(24.0), 2400);
    }

    #[test]
    fn frame_from_px_builds_all_four_fields() {
        let f = Frame::from_px(100.0, 200.0, 300.0, 400.0);
        assert_eq!(f.x_emu, 952_500);
        assert_eq!(f.w_emu, 2_857_500);
    }

    #[test]
    fn roundrect_adj_50_percent_at_half_min_dim() {
        // radius = min_dim / 2 → adj = 50_000 (50%, the cap).
        assert_eq!(roundrect_adj(250_000, 1_000_000, 500_000), 50_000);
    }

    #[test]
    fn roundrect_adj_10_percent_for_50k_radius_in_500k_dim() {
        // 50_000 / 500_000 = 10% → adj = 10_000.
        assert_eq!(roundrect_adj(50_000, 1_000_000, 500_000), 10_000);
    }

    #[test]
    fn roundrect_adj_capped_at_50k() {
        // Radius larger than half the min dim is clamped — PPTX won't render
        // a corner radius larger than 50% of the smaller dimension anyway.
        assert_eq!(roundrect_adj(10_000_000, 1_000_000, 500_000), 50_000);
    }

    #[test]
    fn roundrect_adj_zero_radius_is_zero() {
        assert_eq!(roundrect_adj(0, 1_000_000, 500_000), 0);
    }

    #[test]
    fn roundrect_adj_zero_dim_does_not_panic() {
        // Defensive: a 0×0 frame would div-by-zero without max(1).
        assert_eq!(roundrect_adj(50_000, 0, 0), 50_000);
    }
}
