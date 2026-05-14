//! Phase 6b-03/04 — verify that image data URIs round-trip through
//! from_slide_doc into PptxElement::Image (or full-bleed background).

#![cfg(feature = "slide-doc")]

use kangnam_design_doc_slide::slide::{
    Background as SdBackground, Frame as SdFrame, ImageFit as SdFit, SlideDoc, SlideElement,
};
use kangnam_design_export_pptx::{PptxElement, from_deck, write_deck_to_bytes};

// 1×1 transparent PNG (encoded as base64).
const PNG_1X1: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkAAIAAAoAAv/lxKUAAAAASUVORK5CYII=";

fn build_slide_with_image_data_uri() -> SlideDoc {
    let mut doc = SlideDoc::empty("s1");
    doc.width_px = 1280;
    doc.height_px = 720;
    doc.elements.push(SlideElement::Image {
        id: "i1".into(),
        frame: SdFrame {
            x: 100.0,
            y: 100.0,
            w: 200.0,
            h: 200.0,
        },
        src: format!("data:image/png;base64,{PNG_1X1}"),
        fit: SdFit::Contain,
    });
    doc
}

#[test]
fn data_uri_image_becomes_pptx_image_element() {
    let doc = build_slide_with_image_data_uri();
    use kangnam_design_doc_slide::deck::Deck;
    let deck = Deck {
        id: "test".into(),
        slides: vec![doc],
    };
    let pptx_deck = from_deck(&deck).expect("convert");
    let elements = &pptx_deck.slides[0].elements;
    assert_eq!(elements.len(), 1, "exactly one element");
    match &elements[0] {
        PptxElement::Image(img) => {
            assert!(!img.bytes.is_empty(), "image bytes decoded");
        }
        other => panic!("expected Image, got {other:?}"),
    }
}

#[test]
fn http_image_falls_back_to_transparent_rect() {
    let mut doc = SlideDoc::empty("s1");
    doc.width_px = 1280;
    doc.height_px = 720;
    doc.elements.push(SlideElement::Image {
        id: "i1".into(),
        frame: SdFrame {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        },
        src: "https://example.com/cover.png".into(),
        fit: SdFit::Cover,
    });
    use kangnam_design_doc_slide::deck::Deck;
    let deck = Deck {
        id: "test".into(),
        slides: vec![doc],
    };
    let pptx_deck = from_deck(&deck).expect("convert");
    match &pptx_deck.slides[0].elements[0] {
        PptxElement::Shape(_) => {}
        other => panic!("expected Shape (fallback), got {other:?}"),
    }
}

#[test]
fn background_image_becomes_full_bleed_image_element() {
    let mut doc = SlideDoc::empty("s1");
    doc.width_px = 1280;
    doc.height_px = 720;
    doc.background = SdBackground::Image {
        src: format!("data:image/png;base64,{PNG_1X1}"),
    };
    use kangnam_design_doc_slide::deck::Deck;
    let deck = Deck {
        id: "test".into(),
        slides: vec![doc],
    };
    let pptx_deck = from_deck(&deck).expect("convert");
    let slide = &pptx_deck.slides[0];
    // First element should be the full-bleed background image.
    match &slide.elements[0] {
        PptxElement::Image(img) => {
            // Image fits the full slide (1280×720 → 12_192_000 × 6_858_000 EMU).
            assert!(!img.bytes.is_empty());
        }
        other => panic!("expected full-bleed Image, got {other:?}"),
    }
    // Background falls back to white (Solid).
    use kangnam_design_export_pptx::color::Background;
    matches!(slide.background, Background::Solid { .. });
}

#[test]
fn pptx_with_data_uri_image_writes_successfully() {
    let doc = build_slide_with_image_data_uri();
    use kangnam_design_doc_slide::deck::Deck;
    let deck = Deck {
        id: "test".into(),
        slides: vec![doc],
    };
    let pptx_deck = from_deck(&deck).expect("convert");
    let bytes = write_deck_to_bytes(&pptx_deck).expect("write");
    assert!(bytes.starts_with(b"PK"), "valid zip");
    // Verify the image bytes made it into the PPTX media folder.
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).expect("zip");
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.iter().any(|n| n.starts_with("ppt/media/image")));
}
