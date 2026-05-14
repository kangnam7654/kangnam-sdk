//! Phase 6d-01 — verify `from_html` round-trips into a valid PPTX file.
//!
//! Limited fidelity (option (c) per ADR-013): the v1 implementation
//! handles `<h1>/<h2>/<p>` + body bg + `<img data:...>` + slide-size
//! data-attrs.

use std::io::Read;

use kangnam_design_export_pptx::{from_html, write_deck_to_bytes};

fn entries(bytes: &[u8]) -> Vec<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).expect("zip");
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect()
}

fn read_entry(bytes: &[u8], name: &str) -> Option<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).expect("zip");
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).unwrap();
        if e.name() == name {
            let mut s = String::new();
            e.read_to_string(&mut s).expect("read");
            return Some(s);
        }
    }
    None
}

#[test]
fn html_with_heading_and_body_bg_writes_valid_pptx() {
    let html = r#"<!doctype html>
<html>
<head><title>Demo Deck</title></head>
<body style="background:#1f2937" data-slide-w="1280" data-slide-h="720">
  <h1>Welcome</h1>
  <p>Phase 6d closed-loop demo.</p>
</body>
</html>"#;
    let deck = from_html(html);
    assert_eq!(deck.title.as_deref(), Some("Demo Deck"));
    let bytes = write_deck_to_bytes(&deck).expect("write");
    assert!(bytes.starts_with(b"PK"));
    let names = entries(&bytes);
    assert!(names.iter().any(|n| n == "ppt/slides/slide1.xml"));
    let slide_xml = read_entry(&bytes, "ppt/slides/slide1.xml").expect("slide1");
    assert!(slide_xml.contains("Welcome"));
    assert!(slide_xml.contains("1F2937") || slide_xml.contains("1f2937"));
}

#[test]
fn html_with_data_uri_image_embeds_media() {
    let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkAAIAAAoAAv/lxKUAAAAASUVORK5CYII=";
    let html = format!(r#"<body><h1>x</h1><img src="data:image/png;base64,{png_b64}"></body>"#);
    let deck = from_html(&html);
    let bytes = write_deck_to_bytes(&deck).expect("write");
    let names = entries(&bytes);
    assert!(names.iter().any(|n| n.starts_with("ppt/media/image")));
}

#[test]
fn html_with_oklch_bg_resolves_to_srgb() {
    let html = r#"<body style="background:oklch(0.628 0.258 29.23)"><p>red</p></body>"#;
    let deck = from_html(html);
    let bytes = write_deck_to_bytes(&deck).expect("write");
    let xml = read_entry(&bytes, "ppt/slides/slide1.xml").expect("slide");
    // Approximate red — match the leading high-byte F[ED].
    let upper = xml.to_uppercase();
    assert!(
        upper.contains(r#"VAL="F"#) || upper.contains(r#"VAL="E"#),
        "expected red-ish srgbClr, slide xml = {xml}"
    );
}
