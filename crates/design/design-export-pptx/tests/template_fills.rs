//! Integration tests for v0.3.2 fill variants: tile pattern, radial gradient,
//! linear gradient multi-stop, and the deprecated Gradient desugar.

use std::io::{Cursor, Read};

use design_export_pptx::*;

// ── fixture helpers ────────────────────────────────────────────────────────

fn base_pptx_bytes() -> Vec<u8> {
    let deck = PptxDeck {
        title: Some("FillTests".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck_to_bytes(&deck).expect("base pptx")
}

fn read_zip_entry(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn read_zip_entry_str(bytes: &[u8], name: &str) -> Option<String> {
    String::from_utf8(read_zip_entry(bytes, name)?).ok()
}

fn zip_has_entry_prefix(bytes: &[u8], prefix: &str, suffix: &str) -> bool {
    let archive = zip::ZipArchive::new(Cursor::new(bytes));
    if let Ok(mut a) = archive {
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

// ── test 1: tile pattern embeds PNG in ppt/media/ ─────────────────────────

#[test]
#[allow(deprecated)]
fn tile_pattern_embeds_png_in_media() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();

    let fill = Fill::dot_tile(24, 24, 1.5, [255, 255, 255, 51]).unwrap();
    let png_bytes = match &fill {
        Fill::TilePattern { png_bytes, .. } => png_bytes.clone(),
        _ => panic!("expected TilePattern"),
    };

    tmpl.add_tile_pattern_rect(
        s,
        Frame::from_px(0.0, 0.0, 1280.0, 720.0),
        &png_bytes,
        0,
    )
    .unwrap();

    let bytes = tmpl.pack().unwrap();

    assert!(
        zip_has_entry_prefix(&bytes, "ppt/media/image", ".png"),
        "ppt/media/imageN.png entry must exist in packed zip"
    );
}

// ── test 2: tile pattern blipFill references image rel ────────────────────

#[test]
#[allow(deprecated)]
fn tile_pattern_blip_fill_references_image_rel() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();

    let fill = Fill::dot_tile(24, 24, 1.5, [255, 255, 255, 51]).unwrap();
    let png_bytes = match &fill {
        Fill::TilePattern { png_bytes, .. } => png_bytes.clone(),
        _ => panic!("expected TilePattern"),
    };

    tmpl.add_tile_pattern_rect(
        s,
        Frame::from_px(0.0, 0.0, 1280.0, 720.0),
        &png_bytes,
        0,
    )
    .unwrap();

    let bytes = tmpl.pack().unwrap();

    // slide2 is the appended slide (base has slide1)
    let slide_xml =
        read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").expect("slide2.xml");
    let rels_xml =
        read_zip_entry_str(&bytes, "ppt/slides/_rels/slide2.xml.rels").expect("slide2 rels");

    // Extract the rId from the slide XML
    let embed_start = slide_xml.find(r#"r:embed=""#).expect("r:embed in slide xml");
    let after = &slide_xml[embed_start + r#"r:embed=""#.len()..];
    let embed_end = after.find('"').expect("closing quote");
    let rid = &after[..embed_end];

    assert!(
        slide_xml.contains("<a:blipFill"),
        "slide must have <a:blipFill"
    );
    assert!(
        rels_xml.contains(&format!(r#"Id="{rid}""#)),
        "rels must contain the rId: {rid}"
    );
    assert!(
        rels_xml.contains("relationships/image"),
        "rels type must be image relationship"
    );
}

// ── test 3: radial gradient emits path="circle" ───────────────────────────

#[test]
fn radial_gradient_to_ooxml_emits_path_circle() {
    let stops = vec![
        GradientStop { position: 0.0, color: Color(0, 0, 0), alpha: None },
        GradientStop { position: 0.6, color: Color(50, 50, 50), alpha: Some(30_000) },
        GradientStop { position: 1.0, color: Color(100, 100, 100), alpha: Some(80_000) },
    ];
    let fill = Fill::RadialGradient { stops };
    let xml = fill.to_ooxml_fill_xml();

    assert!(xml.contains(r#"path="circle""#), "must have path circle: {xml}");
    assert!(xml.contains("<a:gradFill"), "must have gradFill: {xml}");
    assert!(xml.contains("<a:gsLst>"), "must have gsLst: {xml}");
}

// ── test 4: radial gradient stops are ordered ─────────────────────────────

#[test]
fn radial_gradient_stops_are_ordered() {
    let stops = vec![
        GradientStop { position: 0.0, color: Color(0, 0, 0), alpha: None },
        GradientStop { position: 0.6, color: Color(50, 50, 50), alpha: Some(30_000) },
        GradientStop { position: 1.0, color: Color(100, 100, 100), alpha: Some(80_000) },
    ];
    let fill = Fill::RadialGradient { stops };
    let xml = fill.to_ooxml_fill_xml();

    // Positions: 0.0 → 0, 0.6 → 60000, 1.0 → 100000
    let pos0 = xml.find(r#"pos="0""#).expect("pos=0 not found");
    let pos60 = xml.find(r#"pos="60000""#).expect("pos=60000 not found");
    let pos100 = xml.find(r#"pos="100000""#).expect("pos=100000 not found");

    assert!(pos0 < pos60, "stop 0 must come before stop 60000");
    assert!(pos60 < pos100, "stop 60000 must come before stop 100000");

    // Check alpha on the 60000 stop
    assert!(
        xml.contains(r#"<a:alpha val="30000"/>"#),
        "30000 alpha tag: {xml}"
    );
}

// ── test 5: linear gradient multi-stop emits all stops ────────────────────

#[test]
fn linear_gradient_multi_stop_emits_all_stops() {
    let stops = vec![
        GradientStop { position: 0.0, color: Color(255, 0, 0), alpha: None },
        GradientStop { position: 0.5, color: Color(0, 255, 0), alpha: None },
        GradientStop { position: 1.0, color: Color(0, 0, 255), alpha: None },
    ];
    let fill = Fill::LinearGradient { angle_deg: 180.0, stops };
    let xml = fill.to_ooxml_fill_xml();

    let gs_count = xml.matches("<a:gs ").count();
    assert_eq!(gs_count, 3, "must emit 3 gradient stops, got {gs_count}: {xml}");

    // CSS 180° → OOXML 90° → 90 × 60_000 = 5_400_000
    assert!(
        xml.contains(r#"ang="5400000""#),
        "CSS 180° → OOXML 5400000: {xml}"
    );
}

// ── test 6: CSS to OOXML angle conversion ─────────────────────────────────

#[test]
fn gradient_angle_css_to_ooxml_conversion() {
    // CSS 0° (up) → OOXML 270° → 270 × 60_000 = 16_200_000
    let fill0 = Fill::LinearGradient {
        angle_deg: 0.0,
        stops: vec![
            GradientStop { position: 0.0, color: Color::BLACK, alpha: None },
            GradientStop { position: 1.0, color: Color::WHITE, alpha: None },
        ],
    };
    assert!(
        fill0.to_ooxml_fill_xml().contains(r#"ang="16200000""#),
        "CSS 0° → 16200000"
    );

    // CSS 90° → OOXML 0°
    let fill90 = Fill::LinearGradient {
        angle_deg: 90.0,
        stops: vec![
            GradientStop { position: 0.0, color: Color::BLACK, alpha: None },
            GradientStop { position: 1.0, color: Color::WHITE, alpha: None },
        ],
    };
    assert!(
        fill90.to_ooxml_fill_xml().contains(r#"ang="0""#),
        "CSS 90° → 0"
    );

    // CSS 180° → OOXML 90° → 5_400_000
    let fill180 = Fill::LinearGradient {
        angle_deg: 180.0,
        stops: vec![
            GradientStop { position: 0.0, color: Color::BLACK, alpha: None },
            GradientStop { position: 1.0, color: Color::WHITE, alpha: None },
        ],
    };
    assert!(
        fill180.to_ooxml_fill_xml().contains(r#"ang="5400000""#),
        "CSS 180° → 5400000"
    );

    // CSS 270° → OOXML 180° → 10_800_000
    let fill270 = Fill::LinearGradient {
        angle_deg: 270.0,
        stops: vec![
            GradientStop { position: 0.0, color: Color::BLACK, alpha: None },
            GradientStop { position: 1.0, color: Color::WHITE, alpha: None },
        ],
    };
    assert!(
        fill270.to_ooxml_fill_xml().contains(r#"ang="10800000""#),
        "CSS 270° → 10800000"
    );
}

// ── test 7: zero dimensions return Err ────────────────────────────────────

#[test]
fn tile_pattern_with_zero_dimensions_returns_err() {
    assert!(
        Fill::dot_tile(0, 10, 1.5, [255, 255, 255, 255]).is_err(),
        "tile_w_px=0 must return Err"
    );
    assert!(
        Fill::dot_tile(10, 0, 1.5, [255, 255, 255, 255]).is_err(),
        "tile_h_px=0 must return Err"
    );
    assert!(
        Fill::dot_tile(10, 10, 1.5, [255, 255, 255, 255]).is_ok(),
        "valid dimensions must return Ok"
    );
}

// ── test 8: deprecated Gradient desugars to LinearGradient ───────────────

#[test]
fn deprecated_gradient_desugars_to_linear() {
    #[allow(deprecated)]
    let legacy = Fill::Gradient {
        from: Color::BLACK,
        to: Color::WHITE,
        angle_deg: 180.0,
    };
    let modern = Fill::LinearGradient {
        angle_deg: 180.0,
        stops: vec![
            GradientStop { position: 0.0, color: Color::BLACK, alpha: None },
            GradientStop { position: 1.0, color: Color::WHITE, alpha: None },
        ],
    };
    assert_eq!(
        legacy.to_ooxml_fill_xml(),
        modern.to_ooxml_fill_xml(),
        "deprecated Gradient must desugar to same XML as equivalent LinearGradient"
    );
}
