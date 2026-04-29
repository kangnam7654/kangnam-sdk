use crate::deck::PptxDeck;
use crate::element::{ImageMime, PptxElement};
use crate::error::PptxWriteError;

use super::xml::{close_elem, empty_elem, into_bytes, new_writer, open_elem, write_decl};

/// Return the full bytes for `[Content_Types].xml`.
pub fn build(deck: &PptxDeck) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Types", &[
        ("xmlns", "http://schemas.openxmlformats.org/package/2006/content-types"),
    ])?;

    // Default extensions: rels, xml, and any image types present in the deck.
    empty_elem(&mut w, "Default", &[
        ("Extension", "rels"),
        ("ContentType", "application/vnd.openxmlformats-package.relationships+xml"),
    ])?;
    empty_elem(&mut w, "Default", &[
        ("Extension", "xml"),
        ("ContentType", "application/xml"),
    ])?;
    let mut used = collect_image_mimes(deck);
    used.sort_by_key(|m| m.ext());
    used.dedup();
    for mime in used {
        empty_elem(&mut w, "Default", &[
            ("Extension", mime.ext()),
            ("ContentType", mime.content_type()),
        ])?;
    }

    // Overrides: presentation.xml, theme, master, layout, slides, core props.
    empty_elem(&mut w, "Override", &[
        ("PartName", "/ppt/presentation.xml"),
        ("ContentType", "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"),
    ])?;
    empty_elem(&mut w, "Override", &[
        ("PartName", "/ppt/theme/theme1.xml"),
        ("ContentType", "application/vnd.openxmlformats-officedocument.theme+xml"),
    ])?;
    empty_elem(&mut w, "Override", &[
        ("PartName", "/ppt/slideMasters/slideMaster1.xml"),
        ("ContentType", "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"),
    ])?;
    empty_elem(&mut w, "Override", &[
        ("PartName", "/ppt/slideLayouts/slideLayout1.xml"),
        ("ContentType", "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"),
    ])?;
    for i in 1..=deck.slides.len() {
        let path = format!("/ppt/slides/slide{i}.xml");
        empty_elem(&mut w, "Override", &[
            ("PartName", &path),
            ("ContentType", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"),
        ])?;
    }
    empty_elem(&mut w, "Override", &[
        ("PartName", "/docProps/core.xml"),
        ("ContentType", "application/vnd.openxmlformats-package.core-properties+xml"),
    ])?;
    empty_elem(&mut w, "Override", &[
        ("PartName", "/docProps/app.xml"),
        ("ContentType", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ])?;

    close_elem(&mut w, "Types")?;
    Ok(into_bytes(w))
}

fn collect_image_mimes(deck: &PptxDeck) -> Vec<ImageMime> {
    deck.slides.iter().flat_map(|s| s.elements.iter()).filter_map(|e| match e {
        PptxElement::Image(img) => Some(img.mime),
        _ => None,
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::PptxSlide;

    #[test]
    fn empty_deck_emits_one_slide_override() {
        let deck = PptxDeck {
            title: None,
            slides: vec![PptxSlide::blank_1280_720()],
        };
        let bytes = build(&deck).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains(r#"PartName="/ppt/slides/slide1.xml""#));
        assert!(s.contains(r#"/docProps/core.xml"#));
        // No image Defaults when no images.
        assert!(!s.contains(r#"Extension="png""#));
    }

    #[test]
    fn deck_with_two_slides_emits_two_slide_overrides() {
        let deck = PptxDeck {
            title: None,
            slides: vec![PptxSlide::blank_1280_720(), PptxSlide::blank_1280_720()],
        };
        let s = String::from_utf8(build(&deck).unwrap()).unwrap();
        assert!(s.contains(r#"/ppt/slides/slide1.xml"#));
        assert!(s.contains(r#"/ppt/slides/slide2.xml"#));
    }
}
