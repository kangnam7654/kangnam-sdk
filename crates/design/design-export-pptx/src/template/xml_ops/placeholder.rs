//! Placeholder discovery in slideLayout XML and slide upsert helpers.

use crate::error::Result;

use super::util::{escape_xml_text, parse_attrs, splice_before, xml_err};

/// Metadata for a `<p:ph>` element found in a slideLayout.
#[derive(Debug, Clone)]
pub(crate) struct PlaceholderMeta {
    pub(crate) idx: u32,
    /// `type="ctrTitle"` etc. None if the `<p:ph>` has no type attribute.
    pub(crate) ph_type: Option<String>,
}

/// Position and size of a placeholder or shape, in EMU.
#[derive(Default, Debug, Clone, Copy)]
pub(crate) struct XfrmRect {
    pub(crate) off_x: i64,
    pub(crate) off_y: i64,
    pub(crate) ext_cx: i64,
    pub(crate) ext_cy: i64,
}

/// Search a slideLayout XML for `<p:ph idx="N"/>` and return its meta.
/// Treats `<p:ph type="ctrTitle"/>` (no idx) as idx=0.
pub(crate) fn find_layout_placeholder(
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

/// Search a slideLayout for the `<p:sp>` carrying placeholder `idx` and extract
/// its `<a:xfrm>` (offset + extent).
pub(crate) fn find_layout_placeholder_xfrm(
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
pub(crate) fn find_sp_ranges(xml: &str) -> Result<Vec<(usize, usize)>> {
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

/// Insert a minimal `<p:txBody>` paragraph chain. Inherits style from layout.
pub(crate) fn build_minimal_tx_body(text: &str) -> String {
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

/// Build a `<p:sp>` containing a placeholder with its text body.
pub(crate) fn build_text_sp_xml(sp_id: usize, ph: &PlaceholderMeta, text: &str) -> String {
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
pub(crate) fn upsert_slide_text_sp(
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

    let next_id = super::slide::next_sp_id_in_slide(slide_xml);
    let sp_xml = build_text_sp_xml(next_id, ph, text);
    splice_before(s, "</p:spTree>", &sp_xml)
}
