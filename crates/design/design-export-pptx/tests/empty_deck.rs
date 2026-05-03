//! Produce a real .pptx for an empty deck and verify structural expectations.

use kangnam_design_export_pptx::*;

#[test]
fn empty_deck_writes_valid_zip_with_expected_parts() {
    let deck = PptxDeck {
        title: Some("Empty".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    let bytes = write_deck_to_bytes(&deck).expect("write deck");

    // Validate it's a real ZIP by parsing the archive.
    let cursor = std::io::Cursor::new(bytes.clone());
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");

    let expected = [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
        "ppt/theme/theme1.xml",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        "ppt/slides/slide1.xml",
        "ppt/slides/_rels/slide1.xml.rels",
    ];
    let names: Vec<_> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    for name in &expected {
        assert!(names.iter().any(|n| n == name), "missing part: {name}");
    }
}

#[test]
fn empty_deck_error_when_no_slides() {
    let deck = PptxDeck { title: None, slides: vec![] };
    let err = write_deck_to_bytes(&deck).unwrap_err();
    assert!(matches!(err, PptxWriteError::EmptyDeck));
}

#[test]
fn write_deck_persists_to_disk() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck(&deck, tmp.path()).unwrap();
    let read_back = std::fs::read(tmp.path()).unwrap();
    assert!(read_back.len() > 500); // non-trivial ZIP
}
