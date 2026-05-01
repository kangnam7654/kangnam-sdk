use crate::deck::PptxDeck;
use crate::error::PptxWriteError;

use super::xml::{close_elem, empty_elem, into_bytes, new_writer, open_elem, write_decl};

const NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// `_rels/.rels` — package-level relationships.
pub fn package_rels() -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Relationships", &[("xmlns", NS)])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId1"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"),
        ("Target", "ppt/presentation.xml"),
    ])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId2"),
        ("Type", "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"),
        ("Target", "docProps/core.xml"),
    ])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId3"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties"),
        ("Target", "docProps/app.xml"),
    ])?;
    close_elem(&mut w, "Relationships")?;
    Ok(into_bytes(w))
}

/// `ppt/_rels/presentation.xml.rels` — presentation → master + theme + each slide.
pub fn presentation_rels(deck: &PptxDeck) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Relationships", &[("xmlns", NS)])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId1"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"),
        ("Target", "slideMasters/slideMaster1.xml"),
    ])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId2"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"),
        ("Target", "theme/theme1.xml"),
    ])?;
    for (i, _) in deck.slides.iter().enumerate() {
        let n = i + 1;
        let rid = format!("rId{}", n + 2);
        let target = format!("slides/slide{n}.xml");
        empty_elem(&mut w, "Relationship", &[
            ("Id", &rid),
            ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"),
            ("Target", &target),
        ])?;
    }
    // notesMaster rel — only when any slide has speaker_notes (Phase 6b-02).
    let any_notes = deck.slides.iter().any(|s| s.speaker_notes.is_some());
    if any_notes {
        let rid = format!("rId{}", deck.slides.len() + 3);
        empty_elem(&mut w, "Relationship", &[
            ("Id", &rid),
            ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster"),
            ("Target", "notesMasters/notesMaster1.xml"),
        ])?;
    }
    close_elem(&mut w, "Relationships")?;
    Ok(into_bytes(w))
}

/// `ppt/slides/_rels/slideN.xml.rels` — slide → layout + embedded
/// images + (optionally) notesSlide.
///
/// Renamed from `slide_rels` in Phase 6b-02 — when `notes_rid` is None
/// and `slide_n` is unused, behavior is identical to the pre-6b output.
pub fn slide_rels_with_notes(
    slide_images: &[(String, String)],
    notes_rid: Option<&str>,
    slide_n: usize,
) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Relationships", &[("xmlns", NS)])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId1"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"),
        ("Target", "../slideLayouts/slideLayout1.xml"),
    ])?;
    for (rid, fname) in slide_images {
        let target = format!("../media/{fname}");
        empty_elem(&mut w, "Relationship", &[
            ("Id", rid.as_str()),
            ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"),
            ("Target", &target),
        ])?;
    }
    if let Some(rid) = notes_rid {
        let target = format!("../notesSlides/notesSlide{slide_n}.xml");
        empty_elem(&mut w, "Relationship", &[
            ("Id", rid),
            ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide"),
            ("Target", &target),
        ])?;
    }
    close_elem(&mut w, "Relationships")?;
    Ok(into_bytes(w))
}

/// `ppt/slideMasters/_rels/slideMaster1.xml.rels` — master → theme + layout.
pub fn master_rels() -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Relationships", &[("xmlns", NS)])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId1"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"),
        ("Target", "../slideLayouts/slideLayout1.xml"),
    ])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId2"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"),
        ("Target", "../theme/theme1.xml"),
    ])?;
    close_elem(&mut w, "Relationships")?;
    Ok(into_bytes(w))
}

/// `ppt/slideLayouts/_rels/slideLayout1.xml.rels` — layout → master.
pub fn layout_rels() -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "Relationships", &[("xmlns", NS)])?;
    empty_elem(&mut w, "Relationship", &[
        ("Id", "rId1"),
        ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"),
        ("Target", "../slideMasters/slideMaster1.xml"),
    ])?;
    close_elem(&mut w, "Relationships")?;
    Ok(into_bytes(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::PptxSlide;

    #[test]
    fn package_rels_points_to_all_three_top_level_parts() {
        let s = String::from_utf8(package_rels().unwrap()).unwrap();
        assert!(s.contains("ppt/presentation.xml"));
        assert!(s.contains("docProps/core.xml"));
        assert!(s.contains("docProps/app.xml"));
    }

    #[test]
    fn presentation_rels_has_one_relationship_per_slide() {
        let d = PptxDeck {
            title: None,
            slides: vec![PptxSlide::blank_1280_720(), PptxSlide::blank_1280_720()],
        };
        let s = String::from_utf8(presentation_rels(&d).unwrap()).unwrap();
        assert!(s.contains(r#"Target="slides/slide1.xml""#));
        assert!(s.contains(r#"Target="slides/slide2.xml""#));
    }

    #[test]
    fn slide_rels_with_no_images_has_only_layout_rel() {
        let s = String::from_utf8(slide_rels_with_notes(&[], None, 1).unwrap()).unwrap();
        assert!(s.contains(r#"Target="../slideLayouts/slideLayout1.xml""#));
        assert!(!s.contains("../media/"));
        assert!(!s.contains("notesSlide"));
    }

    #[test]
    fn slide_rels_with_images_emits_media_targets() {
        let imgs = vec![("rId2".into(), "image1.png".into())];
        let s = String::from_utf8(slide_rels_with_notes(&imgs, None, 1).unwrap()).unwrap();
        assert!(s.contains(r#"Target="../media/image1.png""#));
    }

    #[test]
    fn slide_rels_with_notes_includes_notes_relationship() {
        let s =
            String::from_utf8(slide_rels_with_notes(&[], Some("rId2"), 1).unwrap()).unwrap();
        assert!(s.contains(r#"Target="../notesSlides/notesSlide1.xml""#));
    }
}
