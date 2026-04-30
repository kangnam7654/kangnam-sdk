use crate::color::{Fill, Stroke};
use crate::element::{ShapeBox, ShapeKind};
use crate::error::PptxWriteError;

use super::xml::{close_elem, empty_elem, open_elem, write_raw_fragment, XmlWriter};

pub fn emit(w: &mut XmlWriter, sb: &ShapeBox, sp_id: u32) -> Result<(), PptxWriteError> {
    open_elem(w, "p:sp", &[])?;
    open_elem(w, "p:nvSpPr", &[])?;
    empty_elem(w, "p:cNvPr", &[
        ("id", &sp_id.to_string()),
        ("name", &format!("Shape {sp_id}")),
    ])?;
    open_elem(w, "p:cNvSpPr", &[])?;
    close_elem(w, "p:cNvSpPr")?;
    empty_elem(w, "p:nvPr", &[])?;
    close_elem(w, "p:nvSpPr")?;

    open_elem(w, "p:spPr", &[])?;
    open_elem(w, "a:xfrm", &[])?;
    empty_elem(w, "a:off", &[
        ("x", &sb.frame.x_emu.to_string()),
        ("y", &sb.frame.y_emu.to_string()),
    ])?;
    empty_elem(w, "a:ext", &[
        ("cx", &sb.frame.w_emu.to_string()),
        ("cy", &sb.frame.h_emu.to_string()),
    ])?;
    close_elem(w, "a:xfrm")?;

    emit_geometry(w, &sb.shape, sb.frame.w_emu, sb.frame.h_emu)?;
    emit_fill(w, &sb.fill)?;
    if let Some(stroke) = &sb.stroke {
        emit_stroke(w, stroke)?;
    } else {
        empty_elem(w, "a:ln", &[("w", "0")])?;
    }
    if let Some(shadow) = &sb.shadow {
        // Reuses the same emission helper as the template-edit path, so the
        // two ShapeBox writers produce byte-identical effect XML.
        write_raw_fragment(w, &shadow.to_ooxml_effect_xml())?;
    }
    close_elem(w, "p:spPr")?;

    // Required empty txBody so PowerPoint doesn't complain.
    open_elem(w, "p:txBody", &[])?;
    empty_elem(w, "a:bodyPr", &[])?;
    empty_elem(w, "a:lstStyle", &[])?;
    open_elem(w, "a:p", &[])?;
    empty_elem(w, "a:endParaRPr", &[("lang", "en-US")])?;
    close_elem(w, "a:p")?;
    close_elem(w, "p:txBody")?;

    close_elem(w, "p:sp")?;
    Ok(())
}

fn emit_geometry(
    w: &mut XmlWriter,
    kind: &ShapeKind,
    w_emu: i64,
    h_emu: i64,
) -> Result<(), PptxWriteError> {
    let prst = match kind {
        ShapeKind::Rect => "rect",
        ShapeKind::RoundedRect { .. } => "roundRect",
        ShapeKind::Ellipse => "ellipse",
        ShapeKind::Line => "line",
    };
    open_elem(w, "a:prstGeom", &[("prst", prst)])?;
    open_elem(w, "a:avLst", &[])?;
    if let ShapeKind::RoundedRect { radius_emu } = kind {
        // OOXML adj = corner radius as percent of min(cx,cy) in 1/100_000ths,
        // capped at 50_000 (= 50%). Shared with the template-edit path.
        let adj = crate::geometry::roundrect_adj(*radius_emu, w_emu, h_emu);
        empty_elem(w, "a:gd", &[("name", "adj"), ("fmla", &format!("val {adj}"))])?;
    }
    close_elem(w, "a:avLst")?;
    close_elem(w, "a:prstGeom")?;
    Ok(())
}

fn emit_fill(w: &mut XmlWriter, fill: &Fill) -> Result<(), PptxWriteError> {
    // Delegate to `Fill::to_ooxml_fill_xml` — the canonical emission path
    // shared with the template-edit `add_element(Shape)` flow. The one
    // exception is `TilePattern`, which `to_ooxml_fill_xml` returns as an
    // empty string because it requires slide-context rels mutation. In the
    // write-only `PptxDeck` path we have no rels access, so we substitute
    // `<a:noFill/>` to keep the XML well-formed.
    let fragment = match fill {
        Fill::TilePattern { .. } => "<a:noFill/>".to_string(),
        other => other.to_ooxml_fill_xml(),
    };
    write_raw_fragment(w, &fragment)
}

fn emit_stroke(w: &mut XmlWriter, s: &Stroke) -> Result<(), PptxWriteError> {
    open_elem(w, "a:ln", &[("w", &s.width_emu.to_string())])?;
    open_elem(w, "a:solidFill", &[])?;
    empty_elem(w, "a:srgbClr", &[("val", &s.color.to_hex6())])?;
    close_elem(w, "a:solidFill")?;
    close_elem(w, "a:ln")?;
    Ok(())
}
