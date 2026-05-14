use kangnam_design_export_pptx::*;
use std::io::Read;

#[test]
fn three_slide_deck_has_three_slide_parts_and_three_rels() {
    let mut slides = Vec::new();
    for ch in ["A", "B", "C"] {
        slides.push(PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid {
                color: Color::WHITE,
            },
            elements: vec![PptxElement::Text(TextBox {
                frame: Frame::from_px(500.0, 300.0, 400.0, 100.0),
                content: format!("Slide {ch}"),
                style: {
                    let mut s = TextStyle::default();
                    s.font_size_pt = 48.0;
                    s
                },
            })],

            speaker_notes: None,
        });
    }
    let deck = PptxDeck {
        title: Some("Three".into()),
        slides,
    };
    let bytes = write_deck_to_bytes(&deck).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();

    for n in 1..=3 {
        let mut s = String::new();
        archive
            .by_name(&format!("ppt/slides/slide{n}.xml"))
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        let label = ["A", "B", "C"][n - 1];
        assert!(
            s.contains(&format!("Slide {label}")),
            "slide{n} missing label"
        );
    }
    // Presentation.xml lists three slide IDs
    let mut pres = String::new();
    archive
        .by_name("ppt/presentation.xml")
        .unwrap()
        .read_to_string(&mut pres)
        .unwrap();
    assert!(pres.contains(r#"id="256""#));
    assert!(pres.contains(r#"id="257""#));
    assert!(pres.contains(r#"id="258""#));
}
