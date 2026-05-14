//! Slide XML builders and zip-entry counters.

use crate::error::Result;

use super::relationships::REL_SLIDE_LAYOUT;
use super::util::{splice_before, xml_err};

/// Count slides already present in the loaded template
/// (`ppt/slides/slideN.xml`, excluding `_rels/`).
pub(crate) fn count_existing_slides(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| {
            n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("_rels")
        })
        .count()
}

/// Count media images already present in the loaded template (`ppt/media/imageN.*`).
pub(crate) fn count_existing_images(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| n.starts_with("ppt/media/image"))
        .count()
}

/// Count fonts already embedded in the template (`ppt/fonts/fontN.fntdata`).
/// Used to seed `font_count` at load time so newly embedded fonts get unique IDs.
pub(crate) fn count_existing_fonts(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| n.starts_with("ppt/fonts/font") && n.ends_with(".fntdata"))
        .count()
}

/// Minimal slide XML — empty `<p:spTree>` so that placeholders/shapes can be
/// appended later. Geometry/styling is inherited from the linked slideLayout.
pub(crate) fn build_minimal_slide_xml() -> String {
    concat!(
        r#"<?xml version='1.0' encoding='UTF-8' standalone='yes'?>"#,
        r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#,
        r#" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#,
        r#" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        r#"<p:cSld><p:spTree>"#,
        r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
        r#"<p:grpSpPr/>"#,
        r#"</p:spTree></p:cSld>"#,
        r#"<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"#,
        r#"</p:sld>"#,
    )
    .to_string()
}

/// Build the `.rels` XML for a new slide pointing at `slideLayoutN.xml`.
pub(crate) fn build_slide_rels(layout_num: usize) -> String {
    format!(
        concat!(
            r#"<?xml version='1.0' encoding='UTF-8' standalone='yes'?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="{}" Target="../slideLayouts/slideLayout{}.xml"/>"#,
            r#"</Relationships>"#,
        ),
        REL_SLIDE_LAYOUT, layout_num,
    )
}

/// Insert `content` immediately before `</p:spTree>` in `slide_xml`.
pub(crate) fn insert_before_sptree_close(slide_xml: &[u8], content: &str) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(slide_xml).map_err(|e| xml_err(format!("slide utf-8: {e}")))?;
    splice_before(s, "</p:spTree>", content)
}

/// Compute next available shape `cNvPr id` in the slide as `max(existing) + 1`.
pub(crate) fn next_sp_id_in_slide(slide_xml: &[u8]) -> usize {
    let s = match std::str::from_utf8(slide_xml) {
        Ok(s) => s,
        Err(_) => return 2,
    };
    let mut max_id = 1usize;
    let mut offset = 0;
    while let Some(pos) = s[offset..].find(r#"<p:cNvPr id=""#) {
        let start = offset + pos + r#"<p:cNvPr id=""#.len();
        let end = match s[start..].find('"') {
            Some(i) => start + i,
            None => break,
        };
        if let Ok(n) = s[start..end].parse::<usize>() {
            if n > max_id {
                max_id = n;
            }
        }
        offset = end;
    }
    max_id + 1
}
