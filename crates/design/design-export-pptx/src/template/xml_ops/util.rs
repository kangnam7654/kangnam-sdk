//! Low-level string primitives shared across all xml_ops submodules.

use crate::error::{PptxWriteError, Result};

/// Construct an [`PptxWriteError::Xml`] from a message.
#[inline]
pub(crate) fn xml_err(msg: impl Into<String>) -> PptxWriteError {
    PptxWriteError::Xml(msg.into())
}

/// Extract the value of `name="..."` from an XML tag string.
pub(crate) fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{}=\"", name);
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Insert `insertion` immediately before `marker` in `haystack`, returning UTF-8 bytes.
pub(crate) fn splice_before(haystack: &str, marker: &str, insertion: &str) -> Result<Vec<u8>> {
    let pos = haystack
        .rfind(marker)
        .ok_or_else(|| xml_err(format!("marker '{marker}' missing")))?;
    let mut out = String::with_capacity(haystack.len() + insertion.len());
    out.push_str(&haystack[..pos]);
    out.push_str(insertion);
    out.push_str(&haystack[pos..]);
    Ok(out.into_bytes())
}

/// Scan XML for `Id="rIdN"` attributes and return the highest N found.
pub(crate) fn scan_max_rel_id(xml_str: &str) -> Result<usize> {
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

/// Minimal XML attribute parser: returns `(name, value)` pairs.
pub(crate) fn parse_attrs(s: &str) -> Vec<(String, String)> {
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
        let name = std::str::from_utf8(&bytes[name_start..i])
            .unwrap_or("")
            .to_string();
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
        let val = std::str::from_utf8(&bytes[val_start..i])
            .unwrap_or("")
            .to_string();
        if i < bytes.len() {
            i += 1;
        }
        if !name.is_empty() {
            out.push((name, val));
        }
    }
    out
}

/// Escape XML text content (`&`, `<`, `>`).
pub(crate) fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape XML attribute value (double-quote context).
pub(crate) fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
