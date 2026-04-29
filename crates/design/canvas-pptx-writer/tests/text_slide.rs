use canvas_pptx_writer::*;

#[test]
fn slide_with_text_embeds_characters_in_rich_text() {
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Text(TextBox {
                frame: Frame::from_px(100.0, 100.0, 600.0, 80.0),
                content: "안녕, 캔버스".into(),
                style: {
                    let mut s = TextStyle::default();
                    s.font_family = "Pretendard".into();
                    s.font_size_pt = 32.0;
                    s.font_weight = 700;
                    s.color = Color(0x0E, 0xA5, 0xE9);
                    s
                },
            })],
        }],
    };
    let bytes = write_deck_to_bytes(&deck).unwrap();
    // Extract slide1.xml
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut slide = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("ppt/slides/slide1.xml").unwrap(),
        &mut slide,
    ).unwrap();
    assert!(slide.contains("안녕, 캔버스"), "text content missing");
    assert!(slide.contains(r#"sz="3200""#), "font size 32pt * 100 missing");
    assert!(slide.contains(r#"b="1""#), "bold flag missing");
    assert!(slide.contains(r#"<a:srgbClr val="0EA5E9"/>"#), "color missing");
    assert!(slide.contains(r#"typeface="Pretendard""#), "font-family missing");
}

#[test]
fn text_newlines_become_explicit_line_breaks() {
    let deck = PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Text(TextBox {
                frame: Frame::from_px(0.0, 0.0, 500.0, 100.0),
                content: "line1\nline2".into(),
                style: TextStyle::default(),
            })],
        }],
    };
    let bytes = write_deck_to_bytes(&deck).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut slide = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("ppt/slides/slide1.xml").unwrap(),
        &mut slide,
    ).unwrap();
    // Each `\n` produces a separate <a:p> paragraph (simplest + safest).
    assert_eq!(slide.matches("<a:p>").count(), 2, "need 2 paragraphs");
    assert!(slide.contains("line1"));
    assert!(slide.contains("line2"));
}
