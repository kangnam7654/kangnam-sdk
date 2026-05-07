//! `[Content_Types].xml` read and mutation helpers.

use std::collections::HashSet;

use crate::error::Result;

use super::util::{splice_before, xml_err};

/// OOXML MIME type for a slide part.
pub(crate) const CT_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";

/// OOXML MIME type for embedded font data.
pub(crate) const CT_FONT: &str = "application/x-fontdata";

/// Set of file extensions already declared in `[Content_Types].xml` as
/// `<Default Extension="..."/>`. Used to avoid duplicate Default entries.
pub(crate) fn parse_declared_extensions(
    entries: &[(String, Vec<u8>)],
) -> Result<HashSet<String>> {
    let ct = entries
        .iter()
        .find(|(n, _)| n == "[Content_Types].xml")
        .map(|(_, b)| b.as_slice())
        .ok_or_else(|| xml_err("[Content_Types].xml missing"))?;
    let s = std::str::from_utf8(ct).map_err(|e| xml_err(format!("Content_Types utf-8: {e}")))?;
    let mut set = HashSet::new();
    let mut offset = 0;
    while let Some(pos) = s[offset..].find(r#"<Default Extension=""#) {
        let start = offset + pos + r#"<Default Extension=""#.len();
        let end = s[start..]
            .find('"')
            .map(|i| start + i)
            .ok_or_else(|| xml_err("unterminated Extension attribute"))?;
        set.insert(s[start..end].to_ascii_lowercase());
        offset = end;
    }
    Ok(set)
}

/// Add a `<Override PartName="/ppt/slides/slideN.xml" ContentType="..."/>` for a new slide.
pub(crate) fn insert_content_types_override_for_slide(
    xml: &[u8],
    slide_num: usize,
) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("Content_Types utf-8: {e}")))?;
    let insertion = format!(
        r#"<Override PartName="/ppt/slides/slide{}.xml" ContentType="{}"/>"#,
        slide_num, CT_SLIDE
    );
    splice_before(s, "</Types>", &insertion)
}

/// Add a `<Default Extension="..." ContentType="..."/>` entry (file-type wildcard).
pub(crate) fn insert_content_types_default(xml: &[u8], ext: &str, mime: &str) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("Content_Types utf-8: {e}")))?;
    let insertion = format!(r#"<Default Extension="{}" ContentType="{}"/>"#, ext, mime);
    let start = s
        .find("<Types")
        .ok_or_else(|| xml_err("[Content_Types].xml has no <Types>"))?;
    let after_open = s[start..]
        .find('>')
        .ok_or_else(|| xml_err("<Types> open tag malformed"))?
        + start
        + 1;
    let mut out = String::with_capacity(s.len() + insertion.len());
    out.push_str(&s[..after_open]);
    out.push_str(&insertion);
    out.push_str(&s[after_open..]);
    Ok(out.into_bytes())
}

/// Add a `<Override PartName="..." ContentType="..."/>` entry for a specific part path.
pub(crate) fn insert_content_types_override(
    xml: &[u8],
    part_name: &str,
    content_type: &str,
) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("Content_Types utf-8: {e}")))?;
    let insertion = format!(
        r#"<Override PartName="{}" ContentType="{}"/>"#,
        part_name, content_type
    );
    splice_before(s, "</Types>", &insertion)
}
