//! Convert modern CSS color formats (oklch, display-p3) into sRGB hex
//! that PowerPoint can render. PPTX `srgbClr` is sRGB-only — anything
//! else has to be reduced before write.
//!
//! References:
//! - oklab/oklch: <https://bottosson.github.io/posts/oklab/>
//! - display-p3 → sRGB matrices: CSS Color Module 4 spec
//! - sRGB transfer function: IEC 61966-2-1
//!
//! All intermediate calculations use linear-light values; the final
//! `to_srgb_hex` step applies the sRGB OETF and clips to `[0, 1]`.

use crate::color::Color;

/// Try to parse a CSS color string (any format we support) into a
/// PPTX-friendly `Color`. Returns None for unsupported formats — the
/// caller can decide whether to fall back to a default or error.
///
/// Supports:
/// - `#RRGGBB` / `#RGB` / `#RRGGBBAA` (alpha stripped) via the existing
///   `Color::from_hex` path
/// - `oklch(L C H)` and `oklch(L C H / A)` (alpha ignored)
/// - `oklab(L a b)` and `oklab(L a b / A)`
/// - `color(display-p3 R G B)` and `color(display-p3 R G B / A)`
/// - `rgb(R, G, B)` / `rgba(R, G, B, A)` — alpha stripped
pub fn parse_css_color(s: &str) -> Option<Color> {
    let trimmed = s.trim();
    // Hex shortcuts. Accept #RRGGBB / #RRGGBBAA (alpha stripped) /
    // #RGB shorthand. Existing `Color::from_hex` only handles 6-char,
    // so widen here.
    if let Some(stripped) = trimmed.strip_prefix('#') {
        match stripped.len() {
            6 => return Color::from_hex(stripped),
            8 => return Color::from_hex(&stripped[..6]), // alpha stripped
            3 => {
                // #RGB → #RRGGBB.
                let bytes = stripped.as_bytes();
                let expanded = format!(
                    "{}{}{}{}{}{}",
                    bytes[0] as char,
                    bytes[0] as char,
                    bytes[1] as char,
                    bytes[1] as char,
                    bytes[2] as char,
                    bytes[2] as char,
                );
                return Color::from_hex(&expanded);
            }
            _ => return None,
        }
    }
    // Function form: `name( ... )`
    let (name, args) = parse_func_call(trimmed)?;
    match name.as_str() {
        "oklch" => parse_oklch(&args),
        "oklab" => parse_oklab(&args),
        "color" => parse_color_func(&args),
        "rgb" | "rgba" => parse_rgb(&args),
        _ => None,
    }
}

fn parse_func_call(s: &str) -> Option<(String, String)> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    if close < open {
        return None;
    }
    let name = s[..open].trim().to_ascii_lowercase();
    let args = s[open + 1..close].trim().to_string();
    Some((name, args))
}

/// Split CSS function args on whitespace OR commas, stripping the
/// optional `/ alpha` suffix. Returns the leading numeric tokens.
fn split_args(args: &str) -> Vec<String> {
    let main = args.split('/').next().unwrap_or(args);
    main
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Parse a number that may have a `%` suffix → return the value scaled
/// such that 0..1 means percent (so `100%` → 1.0, `0.5` → 0.5).
fn parse_unit(s: &str, percent_max: f64) -> Option<f64> {
    if let Some(stripped) = s.strip_suffix('%') {
        let v: f64 = stripped.trim().parse().ok()?;
        Some(v / percent_max)
    } else {
        s.parse().ok()
    }
}

fn parse_oklch(args: &str) -> Option<Color> {
    let parts = split_args(args);
    if parts.len() < 3 {
        return None;
    }
    let l = parse_unit(&parts[0], 100.0)?; // 0..1 (100% → 1.0)
    let c = parts[1].parse::<f64>().ok()?;
    let h_deg: f64 = match parts[2].strip_suffix("deg") {
        Some(s) => s.trim().parse().ok()?,
        None => parts[2].parse().ok()?,
    };
    let h_rad = h_deg.to_radians();
    let a = c * h_rad.cos();
    let b = c * h_rad.sin();
    Some(oklab_to_srgb(l, a, b))
}

fn parse_oklab(args: &str) -> Option<Color> {
    let parts = split_args(args);
    if parts.len() < 3 {
        return None;
    }
    let l = parse_unit(&parts[0], 100.0)?;
    let a = parts[1].parse::<f64>().ok()?;
    let b = parts[2].parse::<f64>().ok()?;
    Some(oklab_to_srgb(l, a, b))
}

fn parse_color_func(args: &str) -> Option<Color> {
    // color(<colorspace> <r> <g> <b> [/ alpha])
    let parts = split_args(args);
    if parts.len() < 4 {
        return None;
    }
    let space = parts[0].to_ascii_lowercase();
    let r = parse_unit(&parts[1], 100.0)?;
    let g = parse_unit(&parts[2], 100.0)?;
    let b = parse_unit(&parts[3], 100.0)?;
    match space.as_str() {
        "srgb" => Some(linear_to_srgb_hex_unclamped_already_gamma(r, g, b)),
        "display-p3" => Some(p3_to_srgb(r, g, b)),
        _ => None,
    }
}

fn parse_rgb(args: &str) -> Option<Color> {
    let parts = split_args(args);
    if parts.len() < 3 {
        return None;
    }
    // rgb() bare numbers are already 0..255; `%` form is 0..100 → 0..255.
    let to_byte = |s: &str| -> Option<u8> {
        if let Some(stripped) = s.strip_suffix('%') {
            let v: f64 = stripped.trim().parse().ok()?;
            Some(((v / 100.0).clamp(0.0, 1.0) * 255.0).round() as u8)
        } else {
            let v: f64 = s.parse().ok()?;
            Some(v.clamp(0.0, 255.0).round() as u8)
        }
    };
    Some(Color(to_byte(&parts[0])?, to_byte(&parts[1])?, to_byte(&parts[2])?))
}

/// `linear_to_srgb_hex_unclamped_already_gamma` — when `color(srgb …)`
/// values are *already gamma-encoded* sRGB in 0..1, we just round to
/// 0..255. CSS spec: `color(srgb …)` operates on gamma-encoded sRGB.
fn linear_to_srgb_hex_unclamped_already_gamma(r: f64, g: f64, b: f64) -> Color {
    let to_byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color(to_byte(r), to_byte(g), to_byte(b))
}

/// Convert OKLab → linear sRGB → gamma-encoded sRGB → 8-bit hex.
fn oklab_to_srgb(l: f64, a: f64, b: f64) -> Color {
    // Step 1: oklab → LMS (cubed)
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    // Step 2: cube
    let l_c = l_.powi(3);
    let m_c = m_.powi(3);
    let s_c = s_.powi(3);
    // Step 3: LMS → linear sRGB
    let r =  4.076_741_661_5 * l_c - 3.307_711_591_3 * m_c + 0.230_969_929_2 * s_c;
    let g = -1.268_438_004_6 * l_c + 2.609_757_401_1 * m_c - 0.341_319_396_5 * s_c;
    let b = -0.004_196_086_3 * l_c - 0.703_418_614_7 * m_c + 1.707_614_701_0 * s_c;
    linear_to_srgb_color(r, g, b)
}

/// Convert display-p3 (gamma-encoded) → sRGB → 8-bit hex.
fn p3_to_srgb(r: f64, g: f64, b: f64) -> Color {
    // p3 uses the sRGB OETF (gamma 2.4 with a linear toe).
    let r_lin = srgb_to_linear(r);
    let g_lin = srgb_to_linear(g);
    let b_lin = srgb_to_linear(b);
    // Display-P3 → linear sRGB matrix (CSS Color 4).
    let r_s =  1.224_940_177_2 * r_lin - 0.224_940_177_2 * g_lin + 0.0       * b_lin;
    let g_s = -0.042_056_954_5 * r_lin + 1.042_056_954_5 * g_lin + 0.0       * b_lin;
    let b_s = -0.019_637_554_2 * r_lin - 0.078_636_046_3 * g_lin + 1.098_273_600_5 * b_lin;
    linear_to_srgb_color(r_s, g_s, b_s)
}

fn srgb_to_linear(v: f64) -> f64 {
    let c = v.clamp(0.0, 1.0);
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f64) -> f64 {
    let c = v.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_to_srgb_color(r: f64, g: f64, b: f64) -> Color {
    let to_byte = |v: f64| {
        let g = linear_to_srgb(v);
        (g.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color(to_byte(r), to_byte(g), to_byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: u8, b: u8, tol: i32) -> bool {
        (a as i32 - b as i32).abs() <= tol
    }

    #[test]
    fn hex_pass_through() {
        let c = parse_css_color("#3b82f6").unwrap();
        assert_eq!(c, Color(0x3b, 0x82, 0xf6));
    }

    #[test]
    fn rgb_function() {
        let c = parse_css_color("rgb(255, 0, 128)").unwrap();
        assert_eq!(c, Color(0xff, 0x00, 0x80));
    }

    #[test]
    fn rgba_alpha_stripped() {
        let c = parse_css_color("rgba(0, 128, 255, 0.5)").unwrap();
        assert_eq!(c, Color(0x00, 0x80, 0xff));
    }

    #[test]
    fn oklch_pure_red_approximates_red() {
        // oklch(62.8% 0.258 29.23) ≈ #ff0000
        let c = parse_css_color("oklch(0.628 0.258 29.23)").unwrap();
        // Allow ±2 byte tolerance for floating-point round-trip.
        assert!(approx_eq(c.0, 0xff, 3), "r {} vs 255", c.0);
        assert!(c.1 < 30, "g should be near 0, got {}", c.1);
        assert!(c.2 < 30, "b should be near 0, got {}", c.2);
    }

    #[test]
    fn oklch_percent_and_deg_suffixes() {
        // `62.8%` for L, `29.23deg` for hue.
        let c = parse_css_color("oklch(62.8% 0.258 29.23deg)").unwrap();
        assert!(approx_eq(c.0, 0xff, 3));
    }

    #[test]
    fn oklab_pure_white() {
        let c = parse_css_color("oklab(1 0 0)").unwrap();
        assert_eq!(c.0, 0xff);
        assert_eq!(c.1, 0xff);
        assert_eq!(c.2, 0xff);
    }

    #[test]
    fn display_p3_red_into_srgb_red() {
        // color(display-p3 1 0 0) covers wider gamut but conventional
        // mapping clamps near pure red in sRGB.
        let c = parse_css_color("color(display-p3 1 0 0)").unwrap();
        assert_eq!(c.0, 0xff);
        assert!(c.1 < 30);
        assert!(c.2 < 30);
    }

    #[test]
    fn unknown_function_returns_none() {
        assert!(parse_css_color("hsv(0, 100%, 100%)").is_none());
    }

    #[test]
    fn alpha_slash_form_accepted() {
        let c = parse_css_color("oklch(0.628 0.258 29.23 / 0.5)").unwrap();
        assert!(approx_eq(c.0, 0xff, 3));
    }
}
