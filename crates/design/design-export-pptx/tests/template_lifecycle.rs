//! Integration tests for the `PptxTemplate` lifecycle.
//!
//! These tests build a fixture .pptx in-memory by:
//!   1. Calling `write_deck` to get a valid OOXML package,
//!   2. Optionally rewriting `ppt/slideLayouts/slideLayout1.xml` to include
//!      `<p:ph>` placeholders (since the built-in layout is blank), and
//!   3. Re-zipping.
//!
//! Then exercising `PptxTemplate` against the fixture and asserting on
//! the resulting zip entries / XML content.

use std::io::{Cursor, Read, Write};

use kangnam_design_export_pptx::*;

/// Build a valid 1-slide pptx using the v0.2 write path. Used as the base
/// fixture for round-trip and add-slide tests.
fn base_pptx_bytes() -> Vec<u8> {
    let deck = PptxDeck {
        title: Some("FixtureBase".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck_to_bytes(&deck).expect("base pptx")
}

/// Replace a single zip entry's bytes, preserving order/method.
fn rewrite_entry(pptx: &[u8], target: &str, new_bytes: &[u8]) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(pptx)).expect("zip open");
    let mut out = Cursor::new(Vec::<u8>::new());
    {
        let mut zw = zip::ZipWriter::new(&mut out);
        let opts: zip::write::SimpleFileOptions =
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            zw.start_file(&name, opts).unwrap();
            if name == target {
                zw.write_all(new_bytes).unwrap();
            } else {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).unwrap();
                zw.write_all(&buf).unwrap();
            }
        }
        zw.finish().unwrap();
    }
    out.into_inner()
}

/// Layout XML with one title placeholder (idx omitted, defaults to idx=0)
/// and one body placeholder (idx=1). Geometry is set so `<p:ph idx="1"/>`
/// has discoverable xfrm (used by `set_placeholder_image`).
const LAYOUT_WITH_PLACEHOLDERS: &[u8] = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             type="title" preserve="1">
  <p:cSld name="TitleAndBody">
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title 1"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="ctrTitle"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="838200" y="365125"/><a:ext cx="10515600" cy="1325563"/></a:xfrm>
        </p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Click to edit Master title style</a:t></a:r></a:p></p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Body 2"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="838200" y="1825625"/><a:ext cx="10515600" cy="4351338"/></a:xfrm>
        </p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Click to edit Master text styles</a:t></a:r></a:p></p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sldLayout>
"#;

fn fixture_with_placeholders() -> Vec<u8> {
    rewrite_entry(
        &base_pptx_bytes(),
        "ppt/slideLayouts/slideLayout1.xml",
        LAYOUT_WITH_PLACEHOLDERS,
    )
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

// ── tests ──────────────────────────────────────────────────────────────

#[test]
fn load_then_pack_preserves_entry_count_and_order() {
    let original = base_pptx_bytes();
    let original_archive = zip::ZipArchive::new(Cursor::new(&original)).unwrap();
    let original_n = original_archive.len();
    let original_names: Vec<String> = (0..original_n)
        .map(|i| {
            zip::ZipArchive::new(Cursor::new(&original))
                .unwrap()
                .by_index(i)
                .unwrap()
                .name()
                .to_string()
        })
        .collect();

    let tmpl = PptxTemplate::load_bytes(&original).unwrap();
    let repacked = tmpl.pack().unwrap();

    let archive = zip::ZipArchive::new(Cursor::new(&repacked)).unwrap();
    assert_eq!(archive.len(), original_n, "entry count preserved");
    for (i, expected_name) in original_names.iter().enumerate() {
        let mut a = zip::ZipArchive::new(Cursor::new(&repacked)).unwrap();
        let entry = a.by_index(i).unwrap();
        assert_eq!(entry.name(), expected_name, "entry order preserved at {i}");
    }
}

#[test]
fn untouched_entries_are_byte_identical_after_repack() {
    let original = base_pptx_bytes();
    let tmpl = PptxTemplate::load_bytes(&original).unwrap();
    let repacked = tmpl.pack().unwrap();

    // theme1.xml and slideMaster1.xml are not touched by load/pack.
    for name in [
        "ppt/theme/theme1.xml",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideLayouts/slideLayout1.xml",
    ] {
        let before = read_zip_entry(&original, name).unwrap();
        let after = read_zip_entry(&repacked, name).unwrap();
        assert_eq!(before, after, "{name} bytes preserved");
    }
}

#[test]
fn slide_size_returns_template_dimensions() {
    let tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    // PptxSlide::blank_1280_720() => 1280×720 px slide. EMU = px × 9525.
    let (w, h) = tmpl.slide_size_emu();
    assert_eq!(w, 1280 * 9525);
    assert_eq!(h, 720 * 9525);
}

#[test]
fn layout_count_matches_zip() {
    let tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    assert_eq!(tmpl.layout_count(), 1);
}

#[test]
fn add_slide_from_invalid_layout_returns_typed_error() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let err = tmpl.add_slide_from_layout(99).unwrap_err();
    assert!(
        matches!(
            err,
            PptxWriteError::LayoutNotFound { layout_num: 99, max: 1 }
        ),
        "got {err:?}"
    );
}

#[test]
fn add_slide_from_layout_zero_returns_typed_error() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let err = tmpl.add_slide_from_layout(0).unwrap_err();
    assert!(matches!(err, PptxWriteError::LayoutNotFound { layout_num: 0, .. }));
}

#[test]
fn add_slide_appends_slide_xml_and_rels() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    // Base already has slide1, so the new one is slide2.
    let s = tmpl.add_slide_from_layout(1).unwrap();
    assert_eq!(s, SlideRef(2), "SlideRef is absolute (existing slide1 + appended)");

    let bytes = tmpl.pack().unwrap();

    let slide2 = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").expect("slide2.xml exists");
    assert!(slide2.contains("<p:sld"), "slide2 root is <p:sld>");
    assert!(slide2.contains("<p:spTree>"), "slide2 has spTree");

    let rels2 = read_zip_entry_str(&bytes, "ppt/slides/_rels/slide2.xml.rels")
        .expect("slide2.xml.rels exists");
    assert!(rels2.contains("slideLayouts/slideLayout1.xml"));

    let ct = read_zip_entry_str(&bytes, "[Content_Types].xml").unwrap();
    assert!(
        ct.contains(r#"PartName="/ppt/slides/slide2.xml""#),
        "Content_Types Override added"
    );

    let pres_rels =
        read_zip_entry_str(&bytes, "ppt/_rels/presentation.xml.rels").unwrap();
    assert!(
        pres_rels.contains("slides/slide2.xml"),
        "presentation rels references new slide"
    );

    let pres = read_zip_entry_str(&bytes, "ppt/presentation.xml").unwrap();
    assert!(
        pres.contains("<p:sldIdLst>") && pres.matches("<p:sldId ").count() >= 2,
        "sldIdLst has ≥2 sldId entries (original + new)"
    );
}

#[test]
fn set_placeholder_text_with_blank_layout_returns_not_found() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    let err = tmpl.set_placeholder_text(s, 999, "x").unwrap_err();
    assert!(
        matches!(
            err,
            PptxWriteError::PlaceholderNotFound {
                placeholder_idx: 999,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn set_placeholder_text_with_real_placeholders_injects_runs() {
    let mut tmpl = PptxTemplate::load_bytes(&fixture_with_placeholders()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    tmpl.set_placeholder_text(s, 0, "Hello Title").unwrap();
    tmpl.set_placeholder_text(s, 1, "Body line 1\nBody line 2").unwrap();

    let bytes = tmpl.pack().unwrap();
    let slide2 = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").unwrap();

    assert!(slide2.contains("Hello Title"), "title text injected");
    assert!(slide2.contains("Body line 1"), "body line 1 injected");
    assert!(slide2.contains("Body line 2"), "body line 2 injected");

    // Body has \n → two <a:p> paragraphs in that <p:sp>.
    // Title is one paragraph. Placeholder added by upsert: each new <p:sp>
    // contributes its own <p:txBody>. We therefore expect at least 3 <a:p>.
    assert!(
        slide2.matches("<a:p>").count() >= 3,
        "newline produces extra paragraph; got {} paragraphs",
        slide2.matches("<a:p>").count()
    );

    // Title placeholder type=ctrTitle should be carried over.
    assert!(
        slide2.contains(r#"type="ctrTitle""#),
        "ctrTitle ph type carried into slide"
    );
}

#[test]
fn add_full_bleed_image_inserts_pic_and_image_entry() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    let png = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("test-image.png"),
    )
    .expect("read test-image.png");
    tmpl.add_full_bleed_image(s, &png, ImageMime::Png).unwrap();

    let bytes = tmpl.pack().unwrap();

    // image embedded as ppt/media/imageN.png
    let mut found_image = false;
    for i in 0.. {
        let mut a = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        if i >= a.len() {
            break;
        }
        let entry = a.by_index(i).unwrap();
        if entry.name().starts_with("ppt/media/image") && entry.name().ends_with(".png") {
            found_image = true;
            break;
        }
    }
    assert!(found_image, "media/image*.png entry added");

    let slide2 = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").unwrap();
    assert!(slide2.contains("<p:pic>"), "slide2 has <p:pic>");
    assert!(slide2.contains("r:embed="), "blip has r:embed");

    let rels2 =
        read_zip_entry_str(&bytes, "ppt/slides/_rels/slide2.xml.rels").unwrap();
    assert!(rels2.contains("../media/image"));
    assert!(rels2.contains("relationships/image"));
}

#[test]
fn add_full_bleed_image_uses_slide_dimensions() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let (w, h) = tmpl.slide_size_emu();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    let png = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("test-image.png"),
    )
    .unwrap();
    tmpl.add_full_bleed_image(s, &png, ImageMime::Png).unwrap();

    let bytes = tmpl.pack().unwrap();
    let slide2 = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").unwrap();
    assert!(
        slide2.contains(&format!(r#"<a:ext cx="{w}" cy="{h}"/>"#)),
        "<a:ext> matches slide_size_emu"
    );
}

#[test]
fn add_element_image_uses_frame_coordinates() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    let png = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("test-image.png"),
    )
    .unwrap();
    let img = ImageBox {
        frame: Frame::from_px(100.0, 200.0, 300.0, 400.0),
        bytes: png,
        mime: ImageMime::Png,
        fit: ImageFit::Cover,
    };
    tmpl.add_element(s, PptxElement::Image(img)).unwrap();

    let bytes = tmpl.pack().unwrap();
    let slide2 = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").unwrap();
    // 100px → 952500 EMU, 200px → 1905000 EMU
    assert!(slide2.contains(r#"<a:off x="952500" y="1905000"/>"#));
    // 300px → 2857500 EMU, 400px → 3810000 EMU
    assert!(slide2.contains(r#"<a:ext cx="2857500" cy="3810000"/>"#));
}

/// v0.3.3: add_element(Text) is now fully implemented — this test verifies it
/// succeeds and injects a <p:sp txBox="1"> into the slide XML.
#[test]
fn add_element_text_is_implemented_in_v033() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    let text = TextBox {
        frame: Frame::from_px(10.0, 10.0, 100.0, 50.0),
        content: "hello v0.3.3".into(),
        style: TextStyle::default(),
    };
    tmpl.add_element(s, PptxElement::Text(text)).unwrap();
    let bytes = tmpl.pack().unwrap();
    let slide_xml = read_zip_entry_str(&bytes, "ppt/slides/slide2.xml").unwrap();
    assert!(
        slide_xml.contains(r#"txBox="1""#),
        "slide must contain txBox=\"1\": {slide_xml}"
    );
    assert!(
        slide_xml.contains("hello v0.3.3"),
        "slide must contain text content: {slide_xml}"
    );
}

#[test]
fn pack_after_no_mutation_reproduces_zip_loadable() {
    let original = base_pptx_bytes();
    let tmpl = PptxTemplate::load_bytes(&original).unwrap();
    let repacked = tmpl.pack().unwrap();
    // Re-loadable
    let tmpl2 = PptxTemplate::load_bytes(&repacked).unwrap();
    assert_eq!(tmpl2.layout_count(), 1);
}
