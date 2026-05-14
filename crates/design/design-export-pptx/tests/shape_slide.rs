use kangnam_design_export_pptx::*;

fn slide_with(shape: ShapeKind) -> PptxDeck {
    PptxDeck {
        title: None,
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid {
                color: Color::WHITE,
            },
            elements: vec![PptxElement::Shape(ShapeBox::new(
                Frame::from_px(0.0, 0.0, 200.0, 100.0),
                shape,
                Fill::solid(Color(0x3B, 0x82, 0xF6)),
                None,
            ))],

            speaker_notes: None,
        }],
    }
}

fn slide_xml(deck: &PptxDeck) -> String {
    let bytes = write_deck_to_bytes(deck).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("ppt/slides/slide1.xml").unwrap(),
        &mut s,
    )
    .unwrap();
    s
}

#[test]
fn rect_uses_prst_rect() {
    let d = slide_with(ShapeKind::Rect);
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:prstGeom prst="rect">"#));
    assert!(xml.contains(r#"<a:srgbClr val="3B82F6"/>"#));
}

#[test]
fn rounded_rect_uses_prst_roundrect_with_guide() {
    let d = slide_with(ShapeKind::RoundedRect {
        radius_emu: 190_500,
    });
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:prstGeom prst="roundRect">"#));
    // `adj` guide should appear in avLst — we encode the radius as a percent
    // of the smaller dimension. For now, just assert the guide element is there.
    assert!(xml.contains("<a:gd"));
}

#[test]
fn ellipse_uses_prst_ellipse() {
    let d = slide_with(ShapeKind::Ellipse);
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:prstGeom prst="ellipse">"#));
}

#[test]
fn line_uses_prst_line() {
    let d = slide_with(ShapeKind::Line);
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:prstGeom prst="line">"#));
}

#[test]
fn stroke_emits_ln_element_with_width() {
    let mut d = slide_with(ShapeKind::Rect);
    if let PptxElement::Shape(sb) = &mut d.slides[0].elements[0] {
        sb.stroke = Some(Stroke {
            color: Color::BLACK,
            width_emu: 19_050,
        });
    }
    let xml = slide_xml(&d);
    assert!(xml.contains(r#"<a:ln w="19050">"#));
    assert!(xml.contains(r#"<a:srgbClr val="000000"/>"#));
}

#[test]
fn fill_none_emits_no_fill() {
    let mut d = slide_with(ShapeKind::Rect);
    if let PptxElement::Shape(sb) = &mut d.slides[0].elements[0] {
        sb.fill = Fill::None;
    }
    let xml = slide_xml(&d);
    assert!(xml.contains("<a:noFill/>"));
}

#[test]
fn shadow_emits_effect_lst_in_write_only_path() {
    // Regression: v0.3.4 added ShapeBox::shadow but the write-only
    // path silently dropped it. v0.3.5 wires it through the same emission
    // helper (`OuterShadow::to_ooxml_effect_xml`) used by the template path.
    let mut d = slide_with(ShapeKind::Rect);
    if let PptxElement::Shape(sb) = &mut d.slides[0].elements[0] {
        sb.shadow = Some(OuterShadow {
            dx_px: 4.0,
            dy_px: 4.0,
            blur_px: 12.0,
            color: Color::BLACK,
            alpha: Some(40_000),
        });
    }
    let xml = slide_xml(&d);
    assert!(xml.contains("<a:effectLst>"), "must emit effectLst: {xml}");
    assert!(xml.contains("<a:outerShdw"), "must emit outerShdw: {xml}");
    assert!(
        xml.contains(r#"<a:alpha val="40000"/>"#),
        "must emit alpha: {xml}"
    );
    // dx=dy=4 → atan2(4,4) = 45° → 45 × 60_000 = 2_700_000
    assert!(xml.contains(r#"dir="2700000""#), "shadow direction: {xml}");
}
