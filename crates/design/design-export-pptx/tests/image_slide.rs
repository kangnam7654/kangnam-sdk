use kangnam_design_export_pptx::*;
use std::io::Read;

const TEST_PNG: &[u8] = include_bytes!("fixtures/test-image.png");

#[test]
fn image_slide_embeds_png_in_media_folder() {
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Image(ImageBox {
                frame: Frame::from_px(100.0, 100.0, 200.0, 200.0),
                bytes: TEST_PNG.to_vec(),
                mime: ImageMime::Png,
                fit: ImageFit::Contain,
            })],
        
        speaker_notes: None,
        }],
    };
    let bytes = write_deck_to_bytes(&deck).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();

    // Media file present
    let mut media = Vec::new();
    archive.by_name("ppt/media/image1.png").expect("image missing").read_to_end(&mut media).unwrap();
    assert_eq!(media, TEST_PNG);

    // Slide rels point to it
    let mut rels = String::new();
    archive.by_name("ppt/slides/_rels/slide1.xml.rels").unwrap().read_to_string(&mut rels).unwrap();
    assert!(rels.contains("../media/image1.png"));

    // Slide XML has a <p:pic> referencing the rel
    let mut slide = String::new();
    archive.by_name("ppt/slides/slide1.xml").unwrap().read_to_string(&mut slide).unwrap();
    assert!(slide.contains("<p:pic>"));
    assert!(slide.contains(r#"r:embed="rId2""#));   // rId1 is layout
}

#[test]
fn png_declared_as_jpeg_fails_mime_check() {
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Image(ImageBox {
                frame: Frame::from_px(0.0, 0.0, 10.0, 10.0),
                bytes: TEST_PNG.to_vec(),   // magic bytes = PNG
                mime: ImageMime::Jpeg,       // lie
                fit: ImageFit::Contain,
            })],
        
        speaker_notes: None,
        }],
    };
    let err = write_deck_to_bytes(&deck).unwrap_err();
    assert!(matches!(err, PptxWriteError::MimeMismatch { .. }));
}
