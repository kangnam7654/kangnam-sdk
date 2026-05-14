use crate::element::{TextAlign, TextBox, TextStyle};
use crate::error::PptxWriteError;
use crate::geometry::pt_to_hundredths;

use super::xml::{XmlWriter, close_elem, empty_elem, open_elem, write_text};

/// Emit a `<p:sp>` element for a text box.
/// `sp_id` must be unique within the slide (2+, since `cNvPr id=1` is the groupSp).
pub fn emit(w: &mut XmlWriter, tb: &TextBox, sp_id: u32) -> Result<(), PptxWriteError> {
    open_elem(w, "p:sp", &[])?;

    // nvSpPr
    open_elem(w, "p:nvSpPr", &[])?;
    empty_elem(
        w,
        "p:cNvPr",
        &[
            ("id", &sp_id.to_string()),
            ("name", &format!("TextBox {sp_id}")),
        ],
    )?;
    open_elem(w, "p:cNvSpPr", &[("txBox", "1")])?;
    close_elem(w, "p:cNvSpPr")?;
    empty_elem(w, "p:nvPr", &[])?;
    close_elem(w, "p:nvSpPr")?;

    // spPr (position + geometry)
    open_elem(w, "p:spPr", &[])?;
    open_elem(w, "a:xfrm", &[])?;
    empty_elem(
        w,
        "a:off",
        &[
            ("x", &tb.frame.x_emu.to_string()),
            ("y", &tb.frame.y_emu.to_string()),
        ],
    )?;
    empty_elem(
        w,
        "a:ext",
        &[
            ("cx", &tb.frame.w_emu.to_string()),
            ("cy", &tb.frame.h_emu.to_string()),
        ],
    )?;
    close_elem(w, "a:xfrm")?;
    open_elem(w, "a:prstGeom", &[("prst", "rect")])?;
    empty_elem(w, "a:avLst", &[])?;
    close_elem(w, "a:prstGeom")?;
    empty_elem(w, "a:noFill", &[])?;
    close_elem(w, "p:spPr")?;

    // txBody — the actual text
    open_elem(w, "p:txBody", &[])?;
    let wrap_val = if tb.style.allow_wrap {
        "square"
    } else {
        "none"
    };
    empty_elem(w, "a:bodyPr", &[("wrap", wrap_val), ("rtlCol", "0")])?;
    empty_elem(w, "a:lstStyle", &[])?;
    for line in tb.content.split('\n') {
        emit_paragraph(w, line, &tb.style)?;
    }
    close_elem(w, "p:txBody")?;

    close_elem(w, "p:sp")?;
    Ok(())
}

fn emit_paragraph(w: &mut XmlWriter, text: &str, style: &TextStyle) -> Result<(), PptxWriteError> {
    open_elem(w, "a:p", &[])?;
    let align_val = match style.align {
        TextAlign::Left => "l",
        TextAlign::Center => "ctr",
        TextAlign::Right => "r",
        TextAlign::Justify => "just",
    };

    // <a:pPr algn="..."> optionally contains <a:lnSpc> for line height.
    // Bug fix (v0.2 omission): line_height was stored on TextStyle but never
    // emitted. Now correctly emitted as <a:lnSpc><a:spcPct val="N"/>.
    // OOXML spcPct val units: 1/1000 of a percent → line_height 1.2 = 120000,
    // 0.82 = 82000. Conversion: (line_height * 100_000).round() as u32.
    let lnspc_val = (style.line_height * 100_000.0).round() as u32;
    if lnspc_val > 0 {
        open_elem(w, "a:pPr", &[("algn", align_val)])?;
        open_elem(w, "a:lnSpc", &[])?;
        empty_elem(w, "a:spcPct", &[("val", &lnspc_val.to_string())])?;
        close_elem(w, "a:lnSpc")?;
        close_elem(w, "a:pPr")?;
    } else {
        empty_elem(w, "a:pPr", &[("algn", align_val)])?;
    }

    open_elem(w, "a:r", &[])?;

    // <a:rPr sz="…" b="1" i="1" spc="…" dirty="0">
    let sz = pt_to_hundredths(style.font_size_pt).to_string();
    let bold = if style.font_weight >= 700 { "1" } else { "0" };
    let spc = (style.letter_spacing_pt * 100.0) as i64;
    let mut rpr_attrs: Vec<(&str, String)> = vec![
        ("lang", "en-US".to_string()),
        ("sz", sz),
        ("b", bold.to_string()),
        ("dirty", "0".to_string()),
    ];
    if style.italic {
        rpr_attrs.push(("i", "1".to_string()));
    }
    if spc != 0 {
        rpr_attrs.push(("spc", spc.to_string()));
    }
    let rpr_attrs_ref: Vec<(&str, &str)> =
        rpr_attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    open_elem(w, "a:rPr", &rpr_attrs_ref)?;

    open_elem(w, "a:solidFill", &[])?;
    if let Some(alpha) = style.color_alpha {
        open_elem(w, "a:srgbClr", &[("val", &style.color.to_hex6())])?;
        empty_elem(w, "a:alpha", &[("val", &alpha.to_string())])?;
        close_elem(w, "a:srgbClr")?;
    } else {
        empty_elem(w, "a:srgbClr", &[("val", &style.color.to_hex6())])?;
    }
    close_elem(w, "a:solidFill")?;
    empty_elem(w, "a:latin", &[("typeface", &style.font_family)])?;
    empty_elem(w, "a:ea", &[("typeface", &style.font_family)])?;

    close_elem(w, "a:rPr")?;
    open_elem(w, "a:t", &[])?;
    write_text(w, text)?;
    close_elem(w, "a:t")?;
    close_elem(w, "a:r")?;
    close_elem(w, "a:p")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        super::xml::{into_bytes, new_writer},
        emit,
    };
    use crate::element::{TextBox, TextStyle};
    use crate::geometry::Frame;

    fn render_textbox(tb: &TextBox) -> String {
        let mut w = new_writer();
        emit(&mut w, tb, 2).unwrap();
        String::from_utf8(into_bytes(w)).unwrap()
    }

    #[test]
    fn writer_text_emits_lnspc_for_line_height() {
        // line_height was silently ignored in v0.2 — this test verifies the fix.
        let tb = TextBox {
            frame: Frame::from_px(0.0, 0.0, 100.0, 50.0),
            content: "hello".into(),
            style: TextStyle {
                line_height: 0.82,
                ..Default::default()
            },
        };
        let xml = render_textbox(&tb);
        assert!(
            xml.contains(r#"<a:lnSpc>"#),
            "expected <a:lnSpc> in output:\n{xml}"
        );
        assert!(
            xml.contains(r#"val="82000""#),
            "expected val=\"82000\" for line_height=0.82 (×100_000):\n{xml}"
        );
    }

    #[test]
    fn writer_text_default_line_height_emits_lnspc() {
        // Default line_height = 1.2 → val="120000"
        let tb = TextBox {
            frame: Frame::from_px(0.0, 0.0, 100.0, 50.0),
            content: "default".into(),
            style: TextStyle::default(),
        };
        let xml = render_textbox(&tb);
        assert!(
            xml.contains(r#"val="120000""#),
            "expected val=\"120000\" for default line_height=1.2:\n{xml}"
        );
    }
}
