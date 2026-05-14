//! `.rels` file read and mutation helpers.

use crate::error::Result;

use super::util::{scan_max_rel_id, splice_before, xml_err};

/// Relationship type for a slide part.
pub(crate) const REL_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

/// Relationship type for a slideLayout part.
pub(crate) const REL_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";

/// Relationship type for an image media part.
pub(crate) const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// Relationship type for an embedded font part.
pub(crate) const REL_FONT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font";

/// Compute the next available `rId` integer for a `.rels` file by scanning
/// existing `Id="rIdN"` attributes.
pub(crate) fn compute_next_rel_id(entries: &[(String, Vec<u8>)], path: &str) -> Result<usize> {
    let xml = entries
        .iter()
        .find(|(n, _)| n == path)
        .map(|(_, b)| b.as_slice())
        .ok_or_else(|| xml_err(format!("{path} missing")))?;
    let xml_str = std::str::from_utf8(xml).map_err(|e| xml_err(format!("{path} utf-8: {e}")))?;
    Ok(scan_max_rel_id(xml_str)? + 1)
}

/// Append a `<Relationship Id="rIdN" Type="REL_SLIDE" Target="slides/slideN.xml"/>` to
/// `ppt/_rels/presentation.xml.rels`.
pub(crate) fn append_presentation_rel_for_slide(
    xml: &[u8],
    rel_id: usize,
    slide_num: usize,
) -> Result<Vec<u8>> {
    append_presentation_rel(
        xml,
        rel_id,
        REL_SLIDE,
        &format!("slides/slide{}.xml", slide_num),
    )
}

/// Append an arbitrary `<Relationship>` to `ppt/_rels/presentation.xml.rels`.
pub(crate) fn append_presentation_rel(
    xml: &[u8],
    rel_id: usize,
    rel_type: &str,
    target: &str,
) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("rels utf-8: {e}")))?;
    let insertion = format!(
        r#"<Relationship Id="rId{}" Type="{}" Target="{}"/>"#,
        rel_id, rel_type, target
    );
    splice_before(s, "</Relationships>", &insertion)
}

/// Append a `<Relationship>` to an existing `.rels` XML, returning the new
/// XML and the assigned `rIdN` string.
pub(crate) fn append_relationship(
    rels_xml: &[u8],
    rel_type: &str,
    target: &str,
) -> Result<(Vec<u8>, String)> {
    let s = std::str::from_utf8(rels_xml).map_err(|e| xml_err(format!("rels utf-8: {e}")))?;
    let max_id = scan_max_rel_id(s)?;
    let new_id = format!("rId{}", max_id + 1);
    let insertion = format!(
        r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
        new_id, rel_type, target
    );
    let bytes = splice_before(s, "</Relationships>", &insertion)?;
    Ok((bytes, new_id))
}
