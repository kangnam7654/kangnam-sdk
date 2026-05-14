use crate::deck::PptxDeck;
use crate::error::PptxWriteError;

use super::xml::{
    close_elem, empty_elem, into_bytes, new_writer, open_elem, write_decl, write_text,
};

pub fn core_xml(deck: &PptxDeck) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(
        &mut w,
        "cp:coreProperties",
        &[
            (
                "xmlns:cp",
                "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
            ),
            ("xmlns:dc", "http://purl.org/dc/elements/1.1/"),
            ("xmlns:dcterms", "http://purl.org/dc/terms/"),
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
        ],
    )?;
    open_elem(&mut w, "dc:title", &[])?;
    write_text(&mut w, deck.title.as_deref().unwrap_or(""))?;
    close_elem(&mut w, "dc:title")?;
    open_elem(&mut w, "dc:creator", &[])?;
    write_text(&mut w, "design-export-pptx")?;
    close_elem(&mut w, "dc:creator")?;
    close_elem(&mut w, "cp:coreProperties")?;
    Ok(into_bytes(w))
}

pub fn app_xml(deck: &PptxDeck) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(
        &mut w,
        "Properties",
        &[
            (
                "xmlns",
                "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties",
            ),
            (
                "xmlns:vt",
                "http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes",
            ),
        ],
    )?;
    open_elem(&mut w, "Application", &[])?;
    write_text(&mut w, "design-export-pptx")?;
    close_elem(&mut w, "Application")?;
    open_elem(&mut w, "Slides", &[])?;
    write_text(&mut w, &deck.slides.len().to_string())?;
    close_elem(&mut w, "Slides")?;
    empty_elem(&mut w, "ScaleCrop", &[])?;
    empty_elem(&mut w, "LinksUpToDate", &[])?;
    close_elem(&mut w, "Properties")?;
    Ok(into_bytes(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::PptxSlide;

    #[test]
    fn core_contains_title() {
        let d = PptxDeck {
            title: Some("Hello".into()),
            slides: vec![],
        };
        let s = String::from_utf8(core_xml(&d).unwrap()).unwrap();
        assert!(s.contains("<dc:title>Hello</dc:title>"));
    }

    #[test]
    fn app_reports_slide_count() {
        let d = PptxDeck {
            title: None,
            slides: vec![PptxSlide::blank_1280_720(), PptxSlide::blank_1280_720()],
        };
        let s = String::from_utf8(app_xml(&d).unwrap()).unwrap();
        assert!(s.contains("<Slides>2</Slides>"));
    }
}
