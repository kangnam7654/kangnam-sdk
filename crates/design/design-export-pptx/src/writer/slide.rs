use crate::color::{Background, css_to_ooxml_angle};
use crate::deck::PptxSlide;
use crate::element::PptxElement;
use crate::error::PptxWriteError;

use super::SlideImage;
use super::xml::{close_elem, empty_elem, into_bytes, new_writer, open_elem, write_decl};

/// Build `ppt/slides/slideN.xml`.
pub fn build(slide: &PptxSlide, slide_images: &[SlideImage]) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(
        &mut w,
        "p:sld",
        &[
            (
                "xmlns:a",
                "http://schemas.openxmlformats.org/drawingml/2006/main",
            ),
            (
                "xmlns:r",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
            ),
            (
                "xmlns:p",
                "http://schemas.openxmlformats.org/presentationml/2006/main",
            ),
        ],
    )?;
    open_elem(&mut w, "p:cSld", &[])?;

    // Background
    open_elem(&mut w, "p:bg", &[])?;
    open_elem(&mut w, "p:bgPr", &[])?;
    emit_background_fill(&mut w, &slide.background)?;
    empty_elem(&mut w, "a:effectLst", &[])?;
    close_elem(&mut w, "p:bgPr")?;
    close_elem(&mut w, "p:bg")?;

    // Shape tree
    open_elem(&mut w, "p:spTree", &[])?;
    open_elem(&mut w, "p:nvGrpSpPr", &[])?;
    empty_elem(&mut w, "p:cNvPr", &[("id", "1"), ("name", "")])?;
    empty_elem(&mut w, "p:cNvGrpSpPr", &[])?;
    empty_elem(&mut w, "p:nvPr", &[])?;
    close_elem(&mut w, "p:nvGrpSpPr")?;
    open_elem(&mut w, "p:grpSpPr", &[])?;
    open_elem(&mut w, "a:xfrm", &[])?;
    empty_elem(&mut w, "a:off", &[("x", "0"), ("y", "0")])?;
    empty_elem(&mut w, "a:ext", &[("cx", "0"), ("cy", "0")])?;
    empty_elem(&mut w, "a:chOff", &[("x", "0"), ("y", "0")])?;
    empty_elem(&mut w, "a:chExt", &[("cx", "0"), ("cy", "0")])?;
    close_elem(&mut w, "a:xfrm")?;
    close_elem(&mut w, "p:grpSpPr")?;

    for (idx, element) in slide.elements.iter().enumerate() {
        let sp_id = (idx as u32) + 2; // 1 is reserved for groupSp
        match element {
            PptxElement::Text(tb) => super::text::emit(&mut w, tb, sp_id)?,
            PptxElement::Shape(sb) => super::shape::emit(&mut w, sb, sp_id)?,
            PptxElement::Image(img) => {
                // Look up the rel id assigned during `collect_slide_images`.
                let si = slide_images
                    .iter()
                    .find(|si| si.elem_idx == idx)
                    .expect("slide_images must include every image element");
                super::image::emit_pic(&mut w, img, sp_id, &si.rel_id)?;
            }
        }
    }

    close_elem(&mut w, "p:spTree")?;

    close_elem(&mut w, "p:cSld")?;
    close_elem(&mut w, "p:sld")?;
    Ok(into_bytes(w))
}

fn emit_background_fill(
    w: &mut super::xml::XmlWriter,
    bg: &Background,
) -> Result<(), PptxWriteError> {
    match bg {
        Background::Solid { color } => {
            open_elem(w, "a:solidFill", &[])?;
            empty_elem(w, "a:srgbClr", &[("val", &color.to_hex6())])?;
            close_elem(w, "a:solidFill")?;
        }
        Background::Gradient {
            from,
            to,
            angle_deg,
        } => {
            open_elem(w, "a:gradFill", &[("flip", "none"), ("rotWithShape", "1")])?;
            open_elem(w, "a:gsLst", &[])?;
            open_elem(w, "a:gs", &[("pos", "0")])?;
            empty_elem(w, "a:srgbClr", &[("val", &from.to_hex6())])?;
            close_elem(w, "a:gs")?;
            open_elem(w, "a:gs", &[("pos", "100000")])?;
            empty_elem(w, "a:srgbClr", &[("val", &to.to_hex6())])?;
            close_elem(w, "a:gs")?;
            close_elem(w, "a:gsLst")?;
            // `angle_deg` is CSS convention (0° = up, clockwise) — same as
            // `Fill::LinearGradient`. Bridge through `css_to_ooxml_angle` so
            // SlideDoc → PPTX preserves visual direction. Bug fixed in v0.3.5
            // (was raw `× 60_000`, which silently rotated gradients 90°).
            let ang = css_to_ooxml_angle(*angle_deg);
            empty_elem(w, "a:lin", &[("ang", &ang.to_string()), ("scaled", "0")])?;
            close_elem(w, "a:gradFill")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    #[test]
    fn solid_background_emits_srgbclr() {
        let s = PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid {
                color: Color(0xAB, 0xCD, 0xEF),
            },
            elements: vec![],
            speaker_notes: None,
        };
        let xml = String::from_utf8(build(&s, &[]).unwrap()).unwrap();
        assert!(xml.contains(r#"<a:srgbClr val="ABCDEF"/>"#));
    }

    #[test]
    fn gradient_background_emits_both_stops() {
        let s = PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Gradient {
                from: Color(0x00, 0x00, 0x00),
                to: Color(0xFF, 0xFF, 0xFF),
                angle_deg: 90.0,
            },
            elements: vec![],
            speaker_notes: None,
        };
        let xml = String::from_utf8(build(&s, &[]).unwrap()).unwrap();
        assert!(xml.contains(r#"<a:srgbClr val="000000"/>"#));
        assert!(xml.contains(r#"<a:srgbClr val="FFFFFF"/>"#));
        // CSS 90° = "to right" → OOXML 0° (rightward direction).
        // Formula: (90 - 90 + 360) % 360 = 0; 0 × 60_000 = 0.
        assert!(xml.contains(r#"ang="0""#));
    }
}
