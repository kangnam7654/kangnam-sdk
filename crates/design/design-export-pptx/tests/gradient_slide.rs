use design_export_pptx::*;

fn slide_xml(deck: &PptxDeck) -> String {
    let bytes = write_deck_to_bytes(deck).unwrap();
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut a.by_name("ppt/slides/slide1.xml").unwrap(), &mut s).unwrap();
    s
}

#[test]
fn gradient_background_renders_two_stops_and_angle() {
    let d = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Gradient {
                from: Color(0xFF, 0x00, 0x00),
                to:   Color(0x00, 0x00, 0xFF),
                angle_deg: 45.0,
            },
            elements: vec![],
        }],
    };
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:srgbClr val="FF0000"/>"#));
    assert!(xml.contains(r#"<a:srgbClr val="0000FF"/>"#));
    // CSS 45° (top-left → bottom-right) → OOXML 315° = (45-90+360) mod 360.
    // 315 × 60_000 = 18_900_000. Bug fixed in v0.3.5 (was raw 45 × 60_000 = 2_700_000).
    assert!(xml.contains(r#"ang="18900000""#));
}

#[test]
fn gradient_shape_fill_renders_two_stops() {
    let d = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Shape(ShapeBox::new(
                Frame::from_px(0.0, 0.0, 100.0, 100.0),
                ShapeKind::Rect,
                #[allow(deprecated)]
                Fill::Gradient {
                    from: Color(0x11, 0x22, 0x33),
                    to:   Color(0xAA, 0xBB, 0xCC),
                    angle_deg: 0.0,
                },
                None,
            ))],
        }],
    };
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"val="112233""#));
    assert!(xml.contains(r#"val="AABBCC""#));
}
