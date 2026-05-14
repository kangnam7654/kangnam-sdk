//! `ppt/presentation.xml` read and mutation helpers.

use crate::error::Result;

use super::util::{extract_attr, xml_err};

/// Parse `<p:sldSz cx="..." cy="..."/>` from `ppt/presentation.xml`.
pub(crate) fn parse_slide_size(presentation_xml: &[u8]) -> Option<(i64, i64)> {
    let s = std::str::from_utf8(presentation_xml).ok()?;
    let tag_start = s.find("<p:sldSz")?;
    let tag_end = s[tag_start..].find('>')?;
    let tag = &s[tag_start..tag_start + tag_end];
    let cx = extract_attr(tag, "cx")?.parse::<i64>().ok()?;
    let cy = extract_attr(tag, "cy")?.parse::<i64>().ok()?;
    Some((cx, cy))
}

/// Upsert a font variant into `ppt/presentation.xml`'s `<p:embeddedFontLst>`.
///
/// Three cases:
/// - **Case A**: `<p:embeddedFontLst>` exists AND this `typeface` already has a
///   `<p:embeddedFont>` block — appends `<p:{variant_tag} r:id="rIdN"/>` before
///   the block's closing `</p:embeddedFont>`.
/// - **Case B**: `<p:embeddedFontLst>` exists but this `typeface` is new —
///   inserts a full `<p:embeddedFont>` block before `</p:embeddedFontLst>`.
/// - **Case C**: `<p:embeddedFontLst>` is absent — creates the list with this
///   font, inserting it before `<p:defaultTextStyle>` (or `</p:presentation>`
///   as fallback).
///
/// `variant_tag` must be the exact OOXML lowercase name: `regular`, `bold`,
/// `italic`, or `boldItalic`.
pub(crate) fn upsert_embedded_font(
    xml: &[u8],
    typeface: &str,
    variant_tag: &str,
    rel_id: usize,
) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("presentation utf-8: {e}")))?;

    let variant_elem = format!(r#"<p:{} r:id="rId{}"/>"#, variant_tag, rel_id);
    let font_tag = format!(r#"<p:font typeface="{}"/>"#, typeface);

    let new_s = if let Some(lst_start) = s.find("<p:embeddedFontLst>") {
        let lst_end = s
            .find("</p:embeddedFontLst>")
            .ok_or_else(|| xml_err("<p:embeddedFontLst> open tag without closing tag"))?;
        let section = &s[lst_start..lst_end];

        if let Some(off) = section.find(&font_tag) {
            // Case A: typeface block already exists — splice variant before its </p:embeddedFont>
            let abs_font_end = lst_start + off + font_tag.len();
            let after = &s[abs_font_end..];
            let close_rel = after
                .find("</p:embeddedFont>")
                .ok_or_else(|| xml_err("</p:embeddedFont> closing tag missing"))?;
            let insert_at = abs_font_end + close_rel;
            let mut out = String::with_capacity(s.len() + variant_elem.len());
            out.push_str(&s[..insert_at]);
            out.push_str(&variant_elem);
            out.push_str(&s[insert_at..]);
            out
        } else {
            // Case B: new typeface — insert full block before </p:embeddedFontLst>
            let block = format!(
                "<p:embeddedFont>{}{}</p:embeddedFont>",
                font_tag, variant_elem
            );
            let mut out = String::with_capacity(s.len() + block.len());
            out.push_str(&s[..lst_end]);
            out.push_str(&block);
            out.push_str(&s[lst_end..]);
            out
        }
    } else {
        // Case C: list absent — create it before <p:defaultTextStyle> or </p:presentation>
        let anchor = s
            .find("<p:defaultTextStyle")
            .or_else(|| s.rfind("</p:presentation>"))
            .ok_or_else(|| {
                xml_err(
                    "presentation.xml: neither <p:defaultTextStyle> nor </p:presentation> found",
                )
            })?;
        let block = format!(
            "<p:embeddedFontLst><p:embeddedFont>{}{}</p:embeddedFont></p:embeddedFontLst>",
            font_tag, variant_elem
        );
        let mut out = String::with_capacity(s.len() + block.len());
        out.push_str(&s[..anchor]);
        out.push_str(&block);
        out.push_str(&s[anchor..]);
        out
    };

    Ok(new_s.into_bytes())
}

/// Append a `<p:sldId id="..." r:id="rIdN"/>` entry to `<p:sldIdLst>` in
/// `ppt/presentation.xml`, creating `<p:sldIdLst>` if absent.
pub(crate) fn update_sld_id_lst(xml: &[u8], slide_count: usize, rel_id: usize) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(xml).map_err(|e| xml_err(format!("presentation utf-8: {e}")))?;

    // Slide IDs start at 256 in OOXML, mapped 1:1 to slide order.
    let slide_id = 255 + slide_count;
    let entry = format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, slide_id, rel_id);

    let new_xml = if let Some(start) = s.find("<p:sldIdLst>") {
        let close = s[start..]
            .find("</p:sldIdLst>")
            .ok_or_else(|| xml_err("</p:sldIdLst> missing"))?
            + start;
        let mut out = String::with_capacity(s.len() + entry.len());
        out.push_str(&s[..close]);
        out.push_str(&entry);
        out.push_str(&s[close..]);
        out
    } else {
        let master_close = s
            .find("</p:sldMasterIdLst>")
            .ok_or_else(|| xml_err("</p:sldMasterIdLst> missing"))?
            + "</p:sldMasterIdLst>".len();
        let insertion = format!("<p:sldIdLst>{}</p:sldIdLst>", entry);
        let mut out = String::with_capacity(s.len() + insertion.len());
        out.push_str(&s[..master_close]);
        out.push_str(&insertion);
        out.push_str(&s[master_close..]);
        out
    };

    Ok(new_xml.into_bytes())
}
