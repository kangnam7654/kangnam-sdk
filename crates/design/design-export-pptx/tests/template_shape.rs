//! Integration tests for v0.3.4: add_element(PptxElement::Shape) + OuterShadow.
//!
//! All ShapeBox construction uses the mutation pattern (not struct literal)
//! because ShapeBox is `#[non_exhaustive]` — external crates cannot use struct
//! literal syntax even with `..Default::default()` spread.

use std::io::{Cursor, Read};

use kangnam_design_export_pptx::*;

// ── fixture helpers ────────────────────────────────────────────────────────

fn base_pptx_bytes() -> Vec<u8> {
    let deck = PptxDeck {
        title: Some("ShapeTests".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck_to_bytes(&deck).expect("base pptx")
}

fn read_zip_entry_str(bytes: &[u8], name: &str) -> Option<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut entry = archive.by_name(name).ok()?;
    let mut s = String::new();
    entry.read_to_string(&mut s).ok()?;
    Some(s)
}

fn zip_has_entry_matching(bytes: &[u8], prefix: &str, suffix: &str) -> bool {
    if let Ok(mut a) = zip::ZipArchive::new(Cursor::new(bytes)) {
        for i in 0..a.len() {
            if let Ok(entry) = a.by_index(i) {
                if entry.name().starts_with(prefix) && entry.name().ends_with(suffix) {
                    return true;
                }
            }
        }
    }
    false
}

fn slide2_xml(tmpl: PptxTemplate) -> String {
    let bytes = tmpl.pack().unwrap();
    read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").expect("ppt/slides/slide2.xml must exist")
}

// ── test 1: Rect → prstGeom prst="rect" ──────────────────────────────────

#[test]
fn add_element_shape_rect_emits_prst_geom_rect() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    let sb = ShapeBox::new(
        Frame::from_px(10.0, 10.0, 200.0, 100.0),
        ShapeKind::Rect,
        Fill::solid(Color::BLACK),
        None,
    );
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

    let xml = slide2_xml(tmpl);
    assert!(
        xml.contains(r#"<a:prstGeom prst="rect">"#) || xml.contains(r#"prst="rect""#),
        "must contain prst=rect: {xml}"
    );
    assert!(
        !xml.contains(r#"prst="roundRect""#),
        "must NOT contain roundRect for plain Rect: {xml}"
    );
}

// ── test 2: RoundedRect → avLst with adj ─────────────────────────────────

#[test]
fn add_element_shape_rounded_rect_emits_avlst_adj() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    // frame 1_000_000 × 500_000 EMU, radius = 50_000 EMU
    // adj = (50_000 × 100_000) / 500_000 = 10_000  (i.e. 10% of min dim)
    // Bug fixed in v0.3.5: the formula previously used × 50_000, which
    // produced half the requested corner radius.
    let frame = Frame {
        x_emu: 0,
        y_emu: 0,
        w_emu: 1_000_000,
        h_emu: 500_000,
    };
    let sb = ShapeBox::new(
        frame,
        ShapeKind::RoundedRect { radius_emu: 50_000 },
        Fill::solid(Color::BLACK),
        None,
    );
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

    let xml = slide2_xml(tmpl);
    assert!(
        xml.contains(r#"prst="roundRect""#),
        "must have roundRect: {xml}"
    );
    assert!(xml.contains(r#"<a:avLst>"#), "must have avLst: {xml}");
    assert!(xml.contains(r#"name="adj""#), "must have adj guide: {xml}");
    assert!(
        xml.contains(r#"fmla="val 10000""#),
        "adj must be 10000 (10%): {xml}"
    );
}

// ── test 3: solid fill → solidFill with correct hex ──────────────────────

#[test]
fn add_element_shape_with_solid_fill_emits_solid_fill() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    // Opaque solid (no alpha tag)
    let sb_opaque = ShapeBox::new(
        Frame::from_px(0.0, 0.0, 100.0, 50.0),
        ShapeKind::Rect,
        Fill::solid(Color(0x3B, 0x82, 0xF6)),
        None,
    );
    tmpl.add_element(slide, PptxElement::Shape(sb_opaque))
        .unwrap();

    let xml = slide2_xml(tmpl);
    assert!(
        xml.contains(r#"<a:solidFill>"#),
        "must have solidFill: {xml}"
    );
    assert!(
        xml.contains(r#"val="3B82F6""#),
        "must have correct hex: {xml}"
    );
    assert!(
        !xml.contains("<a:alpha"),
        "must NOT have alpha tag for fully opaque fill: {xml}"
    );

    // Semi-transparent solid (alpha tag present)
    let mut tmpl2 = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide2 = tmpl2.add_slide_from_layout(1).unwrap();
    let sb_alpha = ShapeBox::new(
        Frame::from_px(0.0, 0.0, 100.0, 50.0),
        ShapeKind::Rect,
        Fill::Solid {
            color: Color(0xFF, 0x00, 0x00),
            alpha: Some(50_000),
        },
        None,
    );
    tmpl2
        .add_element(slide2, PptxElement::Shape(sb_alpha))
        .unwrap();
    let xml2 = slide2_xml(tmpl2);
    assert!(
        xml2.contains(r#"<a:alpha val="50000"/>"#),
        "must have alpha tag: {xml2}"
    );
}

// ── test 4: linear gradient fill → gradFill ──────────────────────────────

#[test]
fn add_element_shape_with_gradient_fill_emits_grad_fill() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    let stops = vec![
        GradientStop {
            position: 0.0,
            color: Color(0xFF, 0x00, 0x00),
            alpha: None,
        },
        GradientStop {
            position: 0.5,
            color: Color(0x00, 0xFF, 0x00),
            alpha: None,
        },
        GradientStop {
            position: 1.0,
            color: Color(0x00, 0x00, 0xFF),
            alpha: None,
        },
    ];
    let sb = ShapeBox::new(
        Frame::from_px(0.0, 0.0, 200.0, 100.0),
        ShapeKind::Rect,
        Fill::LinearGradient {
            angle_deg: 180.0,
            stops,
        },
        None,
    );
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

    let xml = slide2_xml(tmpl);
    assert!(xml.contains("<a:gradFill"), "must have gradFill: {xml}");
    assert!(xml.contains("<a:gsLst>"), "must have gsLst: {xml}");
    // 3 gradient stops
    assert_eq!(
        xml.matches("<a:gs ").count(),
        3,
        "must have exactly 3 gs elements: {xml}"
    );
    // CSS 180° → OOXML 90° → 5_400_000
    assert!(
        xml.contains(r#"ang="5400000""#),
        "CSS 180° → OOXML ang=5400000: {xml}"
    );
}

// ── test 5: tile fill embeds PNG + registers rels ─────────────────────────

#[test]
fn add_element_shape_with_tile_fill_embeds_png() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    let fill = Fill::dot_tile(24, 24, 1.5, [255, 255, 255, 51]).unwrap();
    let sb = ShapeBox::new(
        Frame::from_px(0.0, 0.0, 1280.0, 720.0),
        ShapeKind::Rect,
        fill,
        None,
    );
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

    let bytes = tmpl.pack().unwrap();

    // 1. PNG embedded in ppt/media/
    assert!(
        zip_has_entry_matching(&bytes, "ppt/media/image", ".png"),
        "ppt/media/imageN.png must exist in zip"
    );

    let slide_xml = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").expect("slide2.xml");
    let rels_xml =
        read_zip_entry_str(&bytes, "ppt/slides/_rels/slide2.xml.rels").expect("slide2 rels");

    // 2. Slide has blipFill with r:embed
    assert!(
        slide_xml.contains("<a:blipFill"),
        "slide XML must have blipFill: {slide_xml}"
    );
    let embed_start = slide_xml
        .find(r#"r:embed=""#)
        .expect("r:embed must exist in slide XML");
    let after = &slide_xml[embed_start + r#"r:embed=""#.len()..];
    let rid = &after[..after.find('"').expect("closing quote")];

    // 3. Slide rels contains that rId pointing to image relationship
    assert!(
        rels_xml.contains(&format!(r#"Id="{rid}""#)),
        "rels must contain {rid}: {rels_xml}"
    );
    assert!(
        rels_xml.contains("relationships/image"),
        "rels must have image relationship type: {rels_xml}"
    );

    // 4. Tile XML present
    assert!(
        slide_xml.contains("<a:tile"),
        "slide XML must have <a:tile: {slide_xml}"
    );
}

// ── test 6: shadow → outerShdw with correct attrs ────────────────────────

#[test]
fn add_element_shape_with_shadow_emits_outer_shdw() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();

    // dx=4, dy=4, blur=12, color=BLACK, alpha=Some(40_000)
    // blurRad = 12 * 9525 = 114_300
    // dist    = sqrt(4^2 + 4^2) * 9525 = sqrt(32) * 9525 ≈ 53_882
    // dir     = atan2(4, 4) = 45° → 45 * 60_000 = 2_700_000
    let mut sb = ShapeBox::new(
        Frame::from_px(10.0, 10.0, 200.0, 100.0),
        ShapeKind::Rect,
        Fill::solid(Color::BLACK),
        None,
    );
    sb.shadow = Some(OuterShadow {
        dx_px: 4.0,
        dy_px: 4.0,
        blur_px: 12.0,
        color: Color::BLACK,
        alpha: Some(40_000),
    });
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

    let xml = slide2_xml(tmpl);
    assert!(xml.contains("<a:effectLst>"), "must have effectLst: {xml}");
    assert!(xml.contains("<a:outerShdw"), "must have outerShdw: {xml}");
    assert!(
        xml.contains(r#"blurRad="114300""#),
        "blurRad must be 114300: {xml}"
    );
    assert!(xml.contains(r#"dist="53882""#), "dist must be 53882: {xml}");
    assert!(
        xml.contains(r#"dir="2700000""#),
        "dir must be 2700000 (45°): {xml}"
    );
    assert!(
        xml.contains(r#"rotWithShape="0""#),
        "rotWithShape must be 0: {xml}"
    );
    assert!(
        xml.contains(r#"val="000000""#),
        "color must be 000000: {xml}"
    );
    assert!(
        xml.contains(r#"<a:alpha val="40000"/>"#),
        "alpha must be 40000: {xml}"
    );
}

// ── test 7: shadow direction math ─────────────────────────────────────────

#[test]
fn shadow_dir_correctly_computed_from_offsets() {
    // We test outer_shdw_xml indirectly by adding shapes with known offsets
    // and checking the emitted dir attribute.

    let cases: &[(f32, f32, &str)] = &[
        // (dx, dy, expected dir as string)
        (10.0, 0.0, r#"dir="0""#),         // 0°   → 0
        (0.0, 10.0, r#"dir="5400000""#),   // 90°  → 5_400_000
        (-10.0, 0.0, r#"dir="10800000""#), // 180° → 10_800_000
        (0.0, -10.0, r#"dir="16200000""#), // 270° → 16_200_000
    ];

    for (dx, dy, expected_dir) in cases {
        let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
        let slide = tmpl.add_slide_from_layout(1).unwrap();

        let mut sb = ShapeBox::new(
            Frame::from_px(0.0, 0.0, 100.0, 50.0),
            ShapeKind::Rect,
            Fill::solid(Color::BLACK),
            None,
        );
        sb.shadow = Some(OuterShadow {
            dx_px: *dx,
            dy_px: *dy,
            blur_px: 0.0,
            color: Color::BLACK,
            alpha: None,
        });
        tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();

        let xml = slide2_xml(tmpl);
        assert!(
            xml.contains(expected_dir),
            "dx={dx}, dy={dy}: expected {expected_dir} in XML: {xml}"
        );
    }

    // Degenerate case: dist < 0.01 → dir=0, dist=0
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let slide = tmpl.add_slide_from_layout(1).unwrap();
    let mut sb = ShapeBox::new(
        Frame::from_px(0.0, 0.0, 100.0, 50.0),
        ShapeKind::Rect,
        Fill::solid(Color::BLACK),
        None,
    );
    sb.shadow = Some(OuterShadow {
        dx_px: 0.0,
        dy_px: 0.0,
        blur_px: 0.0,
        color: Color::BLACK,
        alpha: None,
    });
    tmpl.add_element(slide, PptxElement::Shape(sb)).unwrap();
    let xml = slide2_xml(tmpl);
    assert!(
        xml.contains(r#"dist="0""#),
        "degenerate offset must have dist=0: {xml}"
    );
    assert!(
        xml.contains(r#"dir="0""#),
        "degenerate offset must have dir=0: {xml}"
    );
}
