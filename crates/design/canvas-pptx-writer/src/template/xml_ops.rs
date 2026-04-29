//! XML manipulation helpers for the template path.
//!
//! These are byte-level string-splice operations on OOXML XML. They are
//! intentionally low-level — the goal is to mutate only the parts that need to
//! change, leaving the rest of the template (master, layouts, theme,
//! embedded fonts, etc.) byte-identical for round-trip preservation.

use std::collections::HashSet;

use crate::error::{PptxWriteError, Result};

pub(super) const CT_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
pub(super) const REL_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
pub(super) const REL_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
pub(super) const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub(super) const REL_FONT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font";
pub(super) const CT_FONT: &str = "application/x-fontdata";

#[inline]
fn xml_err(msg: impl Into<String>) -> PptxWriteError {
    PptxWriteError::Xml(msg.into())
}

// ── presentation.xml metadata ─────────────────────────────────────────

/// Parse `<p:sldSz cx="..." cy="..."/>` from `ppt/presentation.xml`.
pub(super) fn parse_slide_size(presentation_xml: &[u8]) -> Option<(i64, i64)> {
    let s = std::str::from_utf8(presentation_xml).ok()?;
    let tag_start = s.find("<p:sldSz")?;
    let tag_end = s[tag_start..].find('>')?;
    let tag = &s[tag_start..tag_start + tag_end];
    let cx = extract_attr(tag, "cx")?.parse::<i64>().ok()?;
    let cy = extract_attr(tag, "cy")?.parse::<i64>().ok()?;
    Some((cx, cy))
}

fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Compute the next available `rId` integer for a `.rels` file by scanning
/// existing `Id="rIdN"` attributes.
pub(super) fn compute_next_rel_id(
    entries: &[(String, Vec<u8>)],
    path: &str,
) -> Result<usize> {
    let xml = entries
        .iter()
        .find(|(n, _)| n == path)
        .map(|(_, b)| b.as_slice())
        .ok_or_else(|| xml_err(format!("{path} missing")))?;
    let xml_str = std::str::from_utf8(xml).map_err(|e| xml_err(format!("{path} utf-8: {e}")))?;
    Ok(scan_max_rel_id(xml_str)? + 1)
}

fn scan_max_rel_id(xml_str: &str) -> Result<usize> {
    let mut max_id = 0usize;
    let mut offset = 0usize;
    while let Some(pos) = xml_str[offset..].find(r#"Id="rId"#) {
        let start = offset + pos + r#"Id="rId"#.len();
        let end = xml_str[start..]
            .find('"')
            .map(|i| start + i)
            .ok_or_else(|| xml_err("unterminated rId attribute"))?;
        let n: usize = xml_str[start..end]
            .parse()
            .map_err(|e| xml_err(format!("rId parse: {e}")))?;
        if n > max_id {
            max_id = n;
        }
        offset = end;
    }
    Ok(max_id)
}

/// Set of file extensions already declared in `[Content_Types].xml` as
/// `<Default Extension="..."/>`. Used to avoid duplicate Default entries.
pub(super) fn parse_declared_extensions(
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

pub(super) fn count_existing_images(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| n.starts_with("ppt/media/image"))
        .count()
}

/// Count slides already present in the loaded template
/// (`ppt/slides/slideN.xml`, excluding `_rels/`).
pub(super) fn count_existing_slides(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| {
            n.starts_with("ppt/slides/slide")
                && n.ends_with(".xml")
                && !n.contains("_rels")
        })
        .count()
}

/// Count fonts already embedded in the template (`ppt/fonts/fontN.fntdata`).
/// Used to seed `font_count` at load time so newly embedded fonts get unique IDs.
pub(super) fn count_existing_fonts(entries: &[(String, Vec<u8>)]) -> usize {
    entries
        .iter()
        .filter(|(n, _)| {
            n.starts_with("ppt/fonts/font") && n.ends_with(".fntdata")
        })
        .count()
}

// ── slide creation ─────────────────────────────────────────────────────

/// Minimal slide XML — empty `<p:spTree>` so that placeholders/shapes can be
/// appended later. Geometry/styling is inherited from the linked slideLayout.
pub(super) fn build_minimal_slide_xml() -> String {
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

pub(super) fn build_slide_rels(layout_num: usize) -> String {
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

// ── string-splice mutations on existing entries ────────────────────────

pub(super) fn insert_content_types_override_for_slide(
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

pub(super) fn insert_content_types_default(xml: &[u8], ext: &str, mime: &str) -> Result<Vec<u8>> {
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

pub(super) fn append_presentation_rel_for_slide(
    xml: &[u8],
    rel_id: usize,
    slide_num: usize,
) -> Result<Vec<u8>> {
    append_presentation_rel(xml, rel_id, REL_SLIDE, &format!("slides/slide{}.xml", slide_num))
}

/// Append an arbitrary `<Relationship>` to `ppt/_rels/presentation.xml.rels`
/// using a caller-supplied `rel_id`, `rel_type`, and `target`.
pub(super) fn append_presentation_rel(
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

/// Add a `<Override PartName="..." ContentType="..."/>` entry to
/// `[Content_Types].xml`. Unlike `insert_content_types_default` (which adds a
/// `<Default Extension=.../>` for a file-type), this targets a specific part by
/// full path (e.g. `/ppt/fonts/font1.fntdata`).
pub(super) fn insert_content_types_override(
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
pub(super) fn upsert_embedded_font(
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
                xml_err("presentation.xml: neither <p:defaultTextStyle> nor </p:presentation> found")
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

pub(super) fn update_sld_id_lst(xml: &[u8], slide_count: usize, rel_id: usize) -> Result<Vec<u8>> {
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

fn splice_before(haystack: &str, marker: &str, insertion: &str) -> Result<Vec<u8>> {
    let pos = haystack
        .rfind(marker)
        .ok_or_else(|| xml_err(format!("marker '{marker}' missing")))?;
    let mut out = String::with_capacity(haystack.len() + insertion.len());
    out.push_str(&haystack[..pos]);
    out.push_str(insertion);
    out.push_str(&haystack[pos..]);
    Ok(out.into_bytes())
}

// ── slide XML manipulation ─────────────────────────────────────────────

pub(super) fn insert_before_sptree_close(slide_xml: &[u8], content: &str) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(slide_xml).map_err(|e| xml_err(format!("slide utf-8: {e}")))?;
    splice_before(s, "</p:spTree>", content)
}

/// Compute next available shape `cNvPr id` in the slide as `max(existing) + 1`.
pub(super) fn next_sp_id_in_slide(slide_xml: &[u8]) -> usize {
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

/// Append a `<Relationship>` to an existing `.rels` XML, returning the new
/// XML and the assigned `rIdN` string.
pub(super) fn append_relationship(
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

// ── placeholder discovery ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct PlaceholderMeta {
    pub(super) idx: u32,
    /// `type="ctrTitle"` etc. None if the `<p:ph>` has no type attribute.
    pub(super) ph_type: Option<String>,
}

/// Search a slideLayout XML for `<p:ph idx="N"/>` and return its meta.
/// Treats `<p:ph type="ctrTitle"/>` (no idx) as idx=0.
pub(super) fn find_layout_placeholder(
    layout_xml: &[u8],
    placeholder_idx: u32,
) -> Result<Option<PlaceholderMeta>> {
    let s = std::str::from_utf8(layout_xml).map_err(|e| xml_err(format!("layout utf-8: {e}")))?;
    let target_idx = placeholder_idx.to_string();
    let mut offset = 0;
    while let Some(pos) = s[offset..].find("<p:ph") {
        let start = offset + pos;
        let end = s[start..]
            .find('>')
            .map(|i| start + i + 1)
            .ok_or_else(|| xml_err("<p:ph> not closed"))?;
        let tag = &s[start..end];
        let attrs = parse_attrs(tag);
        let idx_attr = attrs.iter().find(|(k, _)| k == "idx").map(|(_, v)| v.clone());
        let ph_type = attrs
            .iter()
            .find(|(k, _)| k == "type")
            .map(|(_, v)| v.clone());
        let matches = match (idx_attr, placeholder_idx) {
            (Some(v), _) => v == target_idx,
            (None, 0) => true,
            _ => false,
        };
        if matches {
            return Ok(Some(PlaceholderMeta {
                idx: placeholder_idx,
                ph_type,
            }));
        }
        offset = end;
    }
    Ok(None)
}

#[derive(Default, Debug, Clone, Copy)]
pub(super) struct XfrmRect {
    pub(super) off_x: i64,
    pub(super) off_y: i64,
    pub(super) ext_cx: i64,
    pub(super) ext_cy: i64,
}

/// Search a slideLayout for the `<p:sp>` carrying placeholder `idx` and extract
/// its `<a:xfrm>` (offset + extent).
pub(super) fn find_layout_placeholder_xfrm(
    layout_xml: &[u8],
    placeholder_idx: u32,
) -> Result<Option<XfrmRect>> {
    let s = std::str::from_utf8(layout_xml).map_err(|e| xml_err(format!("layout utf-8: {e}")))?;
    let target_idx_attr = format!(r#"idx="{}""#, placeholder_idx);
    let sps = find_sp_ranges(s)?;
    for (start, end) in sps {
        let block = &s[start..end];
        let ph_matches = if placeholder_idx == 0 {
            block.contains(&target_idx_attr)
                || (block.contains("<p:ph") && !block.contains("idx="))
        } else {
            block.contains(&target_idx_attr)
        };
        if ph_matches {
            return Ok(Some(extract_xfrm(block)));
        }
    }
    Ok(None)
}

/// Find `<p:sp>...</p:sp>` ranges. OOXML spec disallows nested `<p:sp>`.
pub(super) fn find_sp_ranges(xml: &str) -> Result<Vec<(usize, usize)>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while let Some(pos) = xml[offset..].find("<p:sp>") {
        let start = offset + pos;
        let close_rel = xml[start..]
            .find("</p:sp>")
            .ok_or_else(|| xml_err("</p:sp> missing"))?;
        let end = start + close_rel + "</p:sp>".len();
        ranges.push((start, end));
        offset = end;
    }
    Ok(ranges)
}

fn find_element_range(xml: &str, name: &str) -> Option<(usize, usize)> {
    let open_marker = format!("<{}", name);
    let start = xml.find(&open_marker)?;
    let gt = xml[start..].find('>')? + start;
    if xml.as_bytes().get(gt - 1) == Some(&b'/') {
        return Some((start, gt + 1));
    }
    let close_marker = format!("</{}>", name);
    let close = xml[gt..].find(&close_marker)? + gt;
    Some((start, close + close_marker.len()))
}

fn extract_xfrm(sp_block: &str) -> XfrmRect {
    fn find_attr(s: &str, attr: &str) -> Option<i64> {
        let needle = format!(r#"{}=""#, attr);
        let i = s.find(&needle)?;
        let start = i + needle.len();
        let end = s[start..].find('"')? + start;
        s[start..end].parse().ok()
    }
    let mut r = XfrmRect::default();
    if let Some(sp_pr) = find_element_range(sp_block, "p:spPr") {
        let pr = &sp_block[sp_pr.0..sp_pr.1];
        if let Some((os, oe)) = find_element_range(pr, "a:off") {
            let off = &pr[os..oe];
            r.off_x = find_attr(off, "x").unwrap_or(0);
            r.off_y = find_attr(off, "y").unwrap_or(0);
        }
        if let Some((es, ee)) = find_element_range(pr, "a:ext") {
            let ext = &pr[es..ee];
            r.ext_cx = find_attr(ext, "cx").unwrap_or(5_486_400);
            r.ext_cy = find_attr(ext, "cy").unwrap_or(3_657_600);
        }
    }
    if r.ext_cx == 0 {
        r.ext_cx = 5_486_400;
    }
    if r.ext_cy == 0 {
        r.ext_cy = 3_657_600;
    }
    r
}

// ── attribute parser ───────────────────────────────────────────────────

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = std::str::from_utf8(&bytes[name_start..i]).unwrap_or("").to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            if !name.is_empty() {
                out.push((name, String::new()));
            }
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let val = std::str::from_utf8(&bytes[val_start..i]).unwrap_or("").to_string();
        if i < bytes.len() {
            i += 1;
        }
        if !name.is_empty() {
            out.push((name, val));
        }
    }
    out
}

// ── XML escape ─────────────────────────────────────────────────────────

pub(super) fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

// ── placeholder upsert ─────────────────────────────────────────────────

/// Insert a minimal `<p:txBody>` paragraph chain. Inherits style from layout.
pub(super) fn build_minimal_tx_body(text: &str) -> String {
    let mut out = String::from("<p:txBody><a:bodyPr/><a:lstStyle/>");
    let lines: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.split('\n').collect()
    };
    for line in lines {
        out.push_str("<a:p><a:r><a:rPr lang=\"ko-KR\" altLang=\"en-US\"/><a:t>");
        out.push_str(&escape_xml_text(line));
        out.push_str("</a:t></a:r></a:p>");
    }
    out.push_str("</p:txBody>");
    out
}

fn build_ph_attrs(ph: &PlaceholderMeta) -> String {
    let mut s = String::new();
    if let Some(t) = &ph.ph_type {
        s.push_str(r#" type=""#);
        s.push_str(t);
        s.push('"');
    }
    if ph.idx != 0 {
        s.push_str(r#" idx=""#);
        s.push_str(&ph.idx.to_string());
        s.push('"');
    }
    s
}

pub(super) fn build_text_sp_xml(sp_id: usize, ph: &PlaceholderMeta, text: &str) -> String {
    let ph_attr = build_ph_attrs(ph);
    format!(
        concat!(
            r#"<p:sp>"#,
            r#"<p:nvSpPr>"#,
            r#"<p:cNvPr id="{id}" name="Placeholder {id}"/>"#,
            r#"<p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph{ph_attr}/></p:nvPr>"#,
            r#"</p:nvSpPr>"#,
            r#"<p:spPr/>"#,
            r#"{tx_body}"#,
            r#"</p:sp>"#,
        ),
        id = sp_id,
        ph_attr = ph_attr,
        tx_body = build_minimal_tx_body(text),
    )
}

/// Update existing `<p:sp>` matching placeholder idx, or append a new one.
pub(super) fn upsert_slide_text_sp(
    slide_xml: &[u8],
    ph: &PlaceholderMeta,
    text: &str,
) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(slide_xml).map_err(|e| xml_err(format!("slide utf-8: {e}")))?;
    let target_idx_attr = format!(r#"idx="{}""#, ph.idx);

    let sps = find_sp_ranges(s)?;
    for (sp_start, sp_end) in sps {
        let block = &s[sp_start..sp_end];
        let ph_matches = if ph.idx == 0 {
            block.contains(&target_idx_attr)
                || (block.contains("<p:ph") && !block.contains("idx="))
        } else {
            block.contains(&target_idx_attr)
        };
        if ph_matches {
            let tx_start_rel = block.find("<p:txBody>").ok_or_else(|| {
                xml_err(format!(
                    "<p:txBody> missing in <p:sp> for placeholder idx={}",
                    ph.idx
                ))
            })?;
            let tx_end_rel = block
                .find("</p:txBody>")
                .ok_or_else(|| {
                    xml_err(format!(
                        "</p:txBody> missing in <p:sp> for placeholder idx={}",
                        ph.idx
                    ))
                })?
                + "</p:txBody>".len();
            let new_tx_body = build_minimal_tx_body(text);
            let mut out = String::with_capacity(s.len() + new_tx_body.len());
            out.push_str(&s[..sp_start + tx_start_rel]);
            out.push_str(&new_tx_body);
            out.push_str(&block[tx_end_rel..]);
            out.push_str(&s[sp_end..]);
            return Ok(out.into_bytes());
        }
    }

    let next_id = next_sp_id_in_slide(slide_xml);
    let sp_xml = build_text_sp_xml(next_id, ph, text);
    insert_before_sptree_close(slide_xml, &sp_xml)
}

// ── picture XML ────────────────────────────────────────────────────────

pub(super) fn build_pic_xml(rel_id: &str, pic_seq: usize, xfrm: &XfrmRect) -> String {
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
pub(super) fn build_geom_shape_xml(
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

// ── freeform text box XML ──────────────────────────────────────────────

/// Build a freeform `<p:sp txBox="1">` at absolute EMU coordinates from a
/// [`TextBox`](crate::element::TextBox).
///
/// Ported from dear-jeongbin `build_free_text_box_xml` (export_pptx_ooxml.rs:1401-1509).
pub(super) fn build_free_text_sp_xml(
    sp_id: usize,
    tb: &crate::element::TextBox,
) -> String {
    use crate::element::TextAlign;

    let style = &tb.style;
    let sz_100ths = (style.font_size_pt * 100.0).round() as i64;
    let b_attr = if style.font_weight >= 700 { r#" b="1""# } else { "" };
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

/// Escape XML attribute value (double-quote context).
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── freeform shape XML (v0.3.4) ───────────────────────────────────────────

/// Build `<a:prstGeom …>` XML for the given `ShapeKind`.
///
/// For `RoundedRect`, `adj = clamp((radius_emu × 100_000 / min(w,h)), 0, 50_000)`.
/// OOXML expresses the corner radius as a percentage of the shape's smaller
/// dimension in 1/100_000ths, capped at 50% (= 50_000). Bug fixed in v0.3.5
/// (was `× 50_000`, producing half the requested radius).
pub(super) fn prst_geom_xml(
    kind: &crate::element::ShapeKind,
    w_emu: i64,
    h_emu: i64,
) -> String {
    use crate::element::ShapeKind;
    match kind {
        ShapeKind::Rect => r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#.to_string(),
        ShapeKind::RoundedRect { radius_emu } => {
            let adj = crate::geometry::roundrect_adj(*radius_emu, w_emu, h_emu);
            format!(
                r#"<a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val {adj}"/></a:avLst></a:prstGeom>"#,
            )
        }
        ShapeKind::Ellipse => {
            r#"<a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom>"#.to_string()
        }
        ShapeKind::Line => {
            // TODO(v0.3.5): verify Line round-trip in PowerPoint, may need <p:cxnSp>
            r#"<a:prstGeom prst="line"><a:avLst/></a:prstGeom>"#.to_string()
        }
    }
}

/// Build `<a:ln …>` stroke XML, or `<a:ln><a:noFill/></a:ln>` when `stroke` is `None`.
pub(super) fn stroke_xml(stroke: &Option<crate::color::Stroke>) -> String {
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
pub(super) fn build_shape_sp_xml(
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

pub(super) fn image_mime_for_ext(ext: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_16_9_slide_size() {
        let xml = br#"<?xml version="1.0"?><p:presentation xmlns:p="x">
            <p:sldSz cx="12192000" cy="6858000"/>
        </p:presentation>"#;
        assert_eq!(parse_slide_size(xml), Some((12_192_000, 6_858_000)));
    }

    #[test]
    fn returns_none_when_sldsz_missing() {
        let xml = br#"<p:presentation/>"#;
        assert_eq!(parse_slide_size(xml), None);
    }

    #[test]
    fn build_minimal_tx_body_splits_lines() {
        let body = build_minimal_tx_body("a\nb\nc");
        assert_eq!(body.matches("<a:p>").count(), 3);
        assert!(body.contains(">a<"));
        assert!(body.contains(">b<"));
    }

    #[test]
    fn escape_xml_text_handles_specials() {
        assert_eq!(escape_xml_text("a & b < c > d"), "a &amp; b &lt; c &gt; d");
    }

    #[test]
    fn parse_attrs_handles_multiple() {
        let attrs = parse_attrs(r#"<p:ph type="ctrTitle" idx="0"/>"#);
        assert!(attrs.iter().any(|(k, v)| k == "type" && v == "ctrTitle"));
        assert!(attrs.iter().any(|(k, v)| k == "idx" && v == "0"));
    }

    #[test]
    fn next_sp_id_finds_max_plus_one() {
        let xml = br#"<p:cNvPr id="1"/><p:cNvPr id="42"/><p:cNvPr id="7"/>"#;
        assert_eq!(next_sp_id_in_slide(xml), 43);
    }

    #[test]
    fn append_relationship_returns_next_rid() {
        let rels = br#"<?xml version="1.0"?><Relationships xmlns="x">
            <Relationship Id="rId1" Type="t" Target="a"/>
            <Relationship Id="rId3" Type="t" Target="b"/>
            </Relationships>"#;
        let (out, id) = append_relationship(rels, "type-z", "target-z").unwrap();
        assert_eq!(id, "rId4");
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains(r#"Id="rId4""#));
        assert!(s.contains(r#"Type="type-z""#));
    }
}
