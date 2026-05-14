//! End-to-end test: a deck with speaker_notes emits notesMaster1.xml,
//! notesSlideN.xml, the right content_types overrides, and proper
//! slide → notesSlide rels.

use std::io::Read;

use kangnam_design_export_pptx::{
    color::Background, color::Color, deck::PptxDeck, deck::PptxSlide, write_deck_to_bytes,
};
use zip::ZipArchive;

fn entries(bytes: &[u8]) -> Vec<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = ZipArchive::new(cursor).expect("read zip");
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect()
}

fn read_entry(bytes: &[u8], name: &str) -> Option<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = ZipArchive::new(cursor).expect("read zip");
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        if entry.name() == name {
            let mut s = String::new();
            entry.read_to_string(&mut s).expect("read");
            return Some(s);
        }
    }
    None
}

#[test]
fn deck_without_notes_omits_notes_artifacts() {
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide::blank_1280_720()],
    };
    let bytes = write_deck_to_bytes(&deck).expect("write");
    let names = entries(&bytes);
    assert!(!names.iter().any(|n| n.contains("notesMaster")));
    assert!(!names.iter().any(|n| n.contains("notesSlide")));
}

#[test]
fn deck_with_notes_emits_master_and_per_slide_notes() {
    let mut s1 = PptxSlide::blank_1280_720();
    s1.background = Background::Solid {
        color: Color::WHITE,
    };
    s1.speaker_notes = Some("첫 슬라이드 메모입니다.".into());
    let mut s2 = PptxSlide::blank_1280_720();
    s2.background = Background::Solid {
        color: Color::WHITE,
    };
    let mut s3 = PptxSlide::blank_1280_720();
    s3.background = Background::Solid {
        color: Color::WHITE,
    };
    s3.speaker_notes = Some("Three lines\nof notes\nhere.".into());

    let deck = PptxDeck {
        title: Some("notes-test".into()),
        slides: vec![s1, s2, s3],
    };
    let bytes = write_deck_to_bytes(&deck).expect("write");
    let names = entries(&bytes);

    assert!(
        names
            .iter()
            .any(|n| n == "ppt/notesMasters/notesMaster1.xml")
    );
    assert!(
        names
            .iter()
            .any(|n| n == "ppt/notesMasters/_rels/notesMaster1.xml.rels"),
        "notesMaster rels emitted"
    );
    // Only slides 1 and 3 have notes — no notesSlide2.xml.
    assert!(names.iter().any(|n| n == "ppt/notesSlides/notesSlide1.xml"));
    assert!(!names.iter().any(|n| n == "ppt/notesSlides/notesSlide2.xml"));
    assert!(names.iter().any(|n| n == "ppt/notesSlides/notesSlide3.xml"));

    // Slide 1 rels reference notesSlide.
    let rels = read_entry(&bytes, "ppt/slides/_rels/slide1.xml.rels").expect("slide1 rels");
    assert!(rels.contains("notesSlide1.xml"));

    // Slide 2 rels do NOT reference notesSlide.
    let rels = read_entry(&bytes, "ppt/slides/_rels/slide2.xml.rels").expect("slide2 rels");
    assert!(!rels.contains("notesSlide"));

    // notesSlide1 body contains the Korean text.
    let body = read_entry(&bytes, "ppt/notesSlides/notesSlide1.xml").expect("notes1");
    assert!(body.contains("첫 슬라이드 메모입니다."));

    // Multi-line notes split into multiple <a:p>.
    let body = read_entry(&bytes, "ppt/notesSlides/notesSlide3.xml").expect("notes3");
    assert!(body.matches("<a:p>").count() >= 3);

    // [Content_Types].xml lists overrides for master + per-slide notes.
    let ct = read_entry(&bytes, "[Content_Types].xml").expect("ct");
    assert!(ct.contains("notesMaster1.xml"));
    assert!(ct.contains("notesSlide1.xml"));
    assert!(ct.contains("notesSlide3.xml"));
    assert!(!ct.contains("notesSlide2.xml"));

    // Presentation rels reference notesMaster.
    let prels = read_entry(&bytes, "ppt/_rels/presentation.xml.rels").expect("prels");
    assert!(prels.contains("notesMasters/notesMaster1.xml"));
}
