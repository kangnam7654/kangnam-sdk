//! Shape, picture, and freeform-text-box XML builders.

use super::placeholder::XfrmRect;
use super::util::{escape_xml_attr, escape_xml_text};

/// Map a file extension to its OOXML image MIME type.
pub(crate) fn image_mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// Build a `<p:pic>` element referencing a media relationship.
pub(crate) fn build_pic_xml(rel_id: &str, pic_seq: usize, xfrm: &XfrmRect) -> String {
    // Fixed id base = 1000 + seq to avoid collision with shape ids in the same slide.
    let nv_id = 1000 + pic_seq;
    format!(
        concat!(
            r#"<p:pic>"#,
            r#"<p:nvPicPr>"#,
            r#"<p:cNvPr id="{id}" name="Picture {id}"/>"#,
            r#"<p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>"#,
            r#"<p:nvPr/>"#,
            r#"</p:nvPicPr>"#,
            r#"<p:blipFill>"#,
            r#"<a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="{rid}"/>"#,
            r#"<a:stretch><a:fillRect/></a:stretch>"#,
            r#"</p:blipFill>"#,
            r#"<p:spPr>"#,
            r#"<a:xfrm><a:off x="{ox}" y="{oy}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
            r#"</p:spPr>"#,
            r#"</p:pic>"#,
        ),
        id = nv_id,
        rid = rel_id,
        ox = xfrm.off_x,
        oy = xfrm.off_y,
        cx = xfrm.ext_cx,
        cy = xfrm.ext_cy,
    )
}

/// Build a `<p:sp>` with arbitrary fill XML and optional rounded corners.
///
/// `sp_id` — unique shape id within the slide.
/// `xfrm` — EMU position/size.
/// `border_radius_emu` — if > 0, emits `roundRect` geometry with proportional adj.
/// `fill_xml` — pre-built OOXML fill fragment (e.g. from `blip_tile_fill_xml`).
pub(crate) fn build_geom_shape_xml(
    sp_id: usize,
    xfrm: &XfrmRect,
    border_radius_emu: i64,
    w_emu: i64,
    h_emu: i64,
    fill_xml: &str,
) -> String {
    let prst: &str;
    let avlst: String;
    if border_radius_emu > 0 {
        let min_dim = w_emu.min(h_emu).max(1);
        let pct = (border_radius_emu * 100_000 / min_dim).min(50_000);
        prst = "roundRect";
        avlst = format!(r#"<a:avLst><a:gd name="adj" fmla="val {pct}"/></a:avLst>"#);
    } else {
        prst = "rect";
        avlst = "<a:avLst/>".to_string();
    };
    format!(
        concat!(
            r#"<p:sp>"#,
            r#"<p:nvSpPr>"#,
            r#"<p:cNvPr id="{id}" name="Shape {id}"/>"#,
            r#"<p:cNvSpPr/>"#,
            r#"<p:nvPr/>"#,
            r#"</p:nvSpPr>"#,
            r#"<p:spPr>"#,
            r#"<a:xfrm><a:off x="{ox}" y="{oy}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="{prst}">{avlst}</a:prstGeom>"#,
            r#"{fill}"#,
            r#"<a:ln w="0"/>"#,
            r#"</p:spPr>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody>"#,
            r#"</p:sp>"#,
        ),
        id = sp_id,
        ox = xfrm.off_x,
        oy = xfrm.off_y,
        cx = xfrm.ext_cx,
        cy = xfrm.ext_cy,
        prst = prst,
        avlst = avlst,
        fill = fill_xml,
    )
}

/// Build a freeform `<p:sp txBox="1">` at absolute EMU coordinates from a
/// [`TextBox`](crate::element::TextBox).
///
/// Ported from dear-jeongbin `build_free_text_box_xml` (export_pptx_ooxml.rs:1401-1509).
pub(crate) fn build_free_text_sp_xml(sp_id: usize, tb: &crate::element::TextBox) -> String {
    use crate::element::TextAlign;

    let style = &tb.style;
    let sz_100ths = (style.font_size_pt * 100.0).round() as i64;
    let b_attr = if style.font_weight >= 700 {
        r#" b="1""#
    } else {
        ""
    };
    let i_attr = if style.italic { r#" i="1""# } else { "" };
    let spc_val = (style.letter_spacing_pt * 100.0) as i64;
    let spc_attr = if spc_val != 0 {
        format!(r#" spc="{spc_val}""#)
    } else {
        String::new()
    };

    // line_height: CSS ratio → OOXML spcPct val (1/1000 of a percent).
    // 0.82 → 82000, 1.2 → 120000. emit only when > 0.
    let lnspc_tag = {
        let val = (style.line_height * 100_000.0).round() as u32;
        if val > 0 {
            format!(r#"<a:lnSpc><a:spcPct val="{val}"/></a:lnSpc>"#)
        } else {
            String::new()
        }
    };

    let color_hex = style.color.to_hex6();
    let alpha_tag = match style.color_alpha {
        Some(a) => format!(r#"<a:alpha val="{a}"/>"#),
        None => String::new(),
    };

    let font_family = if style.font_family.trim().is_empty() {
        "Calibri".to_string()
    } else {
        escape_xml_attr(&style.font_family)
    };

    let align_str = match style.align {
        TextAlign::Left => "l",
        TextAlign::Center => "ctr",
        TextAlign::Right => "r",
        TextAlign::Justify => "just",
    };

    let lines: Vec<&str> = if tb.content.is_empty() {
        vec![""]
    } else {
        tb.content.split('\n').collect()
    };

    let mut paras = String::new();
    for line in lines {
        paras.push_str(&format!(
            concat!(
                r#"<a:p>"#,
                r#"<a:pPr algn="{algn}">{lnspc}</a:pPr>"#,
                r#"<a:r>"#,
                r#"<a:rPr lang="ko-KR" altLang="en-US" sz="{sz}"{b}{i}{spc} dirty="0">"#,
                r#"<a:solidFill><a:srgbClr val="{color}">{alpha}</a:srgbClr></a:solidFill>"#,
                r#"<a:latin typeface="{font}"/><a:ea typeface="{font}"/><a:cs typeface="{font}"/>"#,
                r#"</a:rPr>"#,
                r#"<a:t>{txt}</a:t>"#,
                r#"</a:r>"#,
                r#"</a:p>"#,
            ),
            algn = align_str,
            lnspc = lnspc_tag,
            sz = sz_100ths,
            b = b_attr,
            i = i_attr,
            spc = spc_attr,
            color = color_hex,
            alpha = alpha_tag,
            font = font_family,
            txt = escape_xml_text(line),
        ));
    }

    let body_pr = if style.allow_wrap {
        r#"<a:bodyPr wrap="square" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"><a:noAutofit/></a:bodyPr>"#
    } else {
        r#"<a:bodyPr wrap="none" lIns="0" tIns="0" rIns="0" bIns="0" anchor="t"><a:noAutofit/></a:bodyPr>"#
    };

    format!(
        concat!(
            r#"<p:sp>"#,
            r#"<p:nvSpPr>"#,
            r#"<p:cNvPr id="{id}" name="TextBox {id}"/>"#,
            r#"<p:cNvSpPr txBox="1"/>"#,
            r#"<p:nvPr/>"#,
            r#"</p:nvSpPr>"#,
            r#"<p:spPr>"#,
            r#"<a:xfrm><a:off x="{ox}" y="{oy}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
            r#"<a:noFill/>"#,
            r#"<a:ln><a:noFill/></a:ln>"#,
            r#"</p:spPr>"#,
            r#"<p:txBody>"#,
            r#"{body_pr}"#,
            r#"<a:lstStyle/>"#,
            r#"{paras}"#,
            r#"</p:txBody>"#,
            r#"</p:sp>"#,
        ),
        id = sp_id,
        ox = tb.frame.x_emu,
        oy = tb.frame.y_emu,
        cx = tb.frame.w_emu,
        cy = tb.frame.h_emu,
        body_pr = body_pr,
        paras = paras,
    )
}

/// Build `<a:prstGeom …>` XML for the given `ShapeKind`.
///
/// For `RoundedRect`, `adj = clamp((radius_emu × 100_000 / min(w,h)), 0, 50_000)`.
/// OOXML expresses the corner radius as a percentage of the shape's smaller
/// dimension in 1/100_000ths, capped at 50% (= 50_000). Bug fixed in v0.3.5
/// (was `× 50_000`, producing half the requested radius).
pub(crate) fn prst_geom_xml(kind: &crate::element::ShapeKind, w_emu: i64, h_emu: i64) -> String {
    use crate::element::ShapeKind;
    match kind {
        ShapeKind::Rect => r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_string(),
        ShapeKind::RoundedRect { radius_emu } => {
            let adj = crate::geometry::roundrect_adj(*radius_emu, w_emu, h_emu);
            format!(
                r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#,
            )
        }
        ShapeKind::Ellipse => r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_string(),
        ShapeKind::Line => {
            // TODO(v0.3.5): verify Line round-trip in PowerPoint, may need <p:cxnSp>
            r#"<a:prstGeom prst="line"><a:avLst/></a:prstGeom>"#.to_string()
        }
    }
}

/// Build `<a:ln …>` stroke XML, or `<a:ln><a:noFill/></a:ln>` when `stroke` is `None`.
pub(crate) fn stroke_xml(stroke: &Option<crate::color::Stroke>) -> String {
    match stroke {
        Some(s) => format!(
            r#"<a:ln w="{w}"><a:solidFill><a:srgbClr val="{hex}"/></a:solidFill></a:ln>"#,
            w = s.width_emu,
            hex = s.color.to_hex6(),
        ),
        None => r#"<a:ln><a:noFill/></a:ln>"#.to_string(),
    }
}

/// Build a full `<p:sp>` for a freeform shape (geometry + fill + effect + stroke).
///
/// - `geom_xml` — from [`prst_geom_xml`].
/// - `fill_xml` — from `Fill::to_ooxml_fill_xml()` or `blip_tile_fill_xml()`.
/// - `stroke_xml` — from [`stroke_xml`].
/// - `effect_xml` — from `OuterShadow::to_ooxml_effect_xml`, or empty string for no effect.
pub(crate) fn build_shape_sp_xml(
    sp_id: usize,
    xfrm: &XfrmRect,
    geom_xml: &str,
    fill_xml: &str,
    stroke_xml: &str,
    effect_xml: &str,
) -> String {
    format!(
        concat!(
            r#"<p:sp>"#,
            r#"<p:nvSpPr>"#,
            r#"<p:cNvPr id="{id}" name="Shape {id}"/>"#,
            r#"<p:cNvSpPr/>"#,
            r#"<p:nvPr/>"#,
            r#"</p:nvSpPr>"#,
            r#"<p:spPr>"#,
            r#"<a:xfrm><a:off x="{ox}" y="{oy}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"{geom}"#,
            r#"{fill}"#,
            r#"{stroke}"#,
            r#"{effect}"#,
            r#"</p:spPr>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody>"#,
            r#"</p:sp>"#,
        ),
        id = sp_id,
        ox = xfrm.off_x,
        oy = xfrm.off_y,
        cx = xfrm.ext_cx,
        cy = xfrm.ext_cy,
        geom = geom_xml,
        fill = fill_xml,
        stroke = stroke_xml,
        effect = effect_xml,
    )
}
