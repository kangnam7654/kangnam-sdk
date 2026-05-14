//! Tiny wrappers over `quick_xml::Writer` for common OOXML patterns.

use std::io::Cursor;

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};

use crate::error::PptxWriteError;

pub type XmlWriter = Writer<Cursor<Vec<u8>>>;

pub fn new_writer() -> XmlWriter {
    Writer::new(Cursor::new(Vec::new()))
}

pub fn write_decl(w: &mut XmlWriter) -> Result<(), PptxWriteError> {
    w.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))
    .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

/// Write `<name attr1="v1" attr2="v2" />` — self-closing.
pub fn empty_elem(
    w: &mut XmlWriter,
    name: &str,
    attrs: &[(&str, &str)],
) -> Result<(), PptxWriteError> {
    let mut start = BytesStart::new(name);
    for (k, v) in attrs {
        start.push_attribute((*k, *v));
    }
    w.write_event(Event::Empty(start))
        .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

/// Open tag `<name attr1="v1" …>`.
pub fn open_elem(
    w: &mut XmlWriter,
    name: &str,
    attrs: &[(&str, &str)],
) -> Result<(), PptxWriteError> {
    let mut start = BytesStart::new(name);
    for (k, v) in attrs {
        start.push_attribute((*k, *v));
    }
    w.write_event(Event::Start(start))
        .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

pub fn close_elem(w: &mut XmlWriter, name: &str) -> Result<(), PptxWriteError> {
    w.write_event(Event::End(BytesEnd::new(name)))
        .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

pub fn write_text(w: &mut XmlWriter, text: &str) -> Result<(), PptxWriteError> {
    w.write_event(Event::Text(BytesText::new(text)))
        .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

pub fn into_bytes(w: XmlWriter) -> Vec<u8> {
    w.into_inner().into_inner()
}

/// Write a pre-built XML fragment verbatim into the writer's output buffer.
/// Used when building a fragment via string formatting is simpler than calling
/// the individual elem helpers (e.g. for gradient / fill XML).
pub fn write_raw_fragment(w: &mut XmlWriter, fragment: &str) -> Result<(), PptxWriteError> {
    use std::io::Write as IoWrite;
    w.get_mut()
        .write_all(fragment.as_bytes())
        .map_err(|e| PptxWriteError::Xml(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_document_emits_decl_and_nested_elems() {
        let mut w = new_writer();
        write_decl(&mut w).unwrap();
        open_elem(&mut w, "root", &[("xmlns", "urn:x")]).unwrap();
        empty_elem(&mut w, "child", &[("a", "1")]).unwrap();
        close_elem(&mut w, "root").unwrap();
        let s = String::from_utf8(into_bytes(w)).unwrap();
        assert!(s.starts_with(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#));
        assert!(s.contains(r#"<root xmlns="urn:x">"#));
        assert!(s.contains(r#"<child a="1"/>"#));
        assert!(s.ends_with("</root>"));
    }
}
