//! Integration tests for `PptxTemplate::embed_font` (v0.3.1).
//!
//! Each test builds a base PPTX in-memory via `write_deck_to_bytes`, then calls
//! `embed_font` one or more times and asserts on the resulting zip entries and
//! XML content.

use std::io::{Cursor, Read};

use kangnam_design_export_pptx::*;

// ── fixture helpers ────────────────────────────────────────────────────

/// Build a valid 1-slide PPTX using the v0.2 write path.
fn base_pptx_bytes() -> Vec<u8> {
    let deck = PptxDeck {
        title: Some("FontEmbedFixture".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck_to_bytes(&deck).expect("base pptx")
}

/// 16-byte sentinel that stands in for a real TTF. The `embed_font` API stores
/// bytes verbatim without validating font structure, so this is sufficient for
/// testing zip/XML mutations.
fn fake_ttf_bytes() -> Vec<u8> {
    b"FAKETTFFAKETTFFA".to_vec()
}

/// Check whether a packed PPTX zip contains an entry with the given name.
fn zip_has_entry(bytes: &[u8], name: &str) -> bool {
    let mut archive = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(a) => a,
        Err(_) => return false,
    };
    archive.by_name(name).is_ok()
}

/// Read a zip entry as a UTF-8 string. Returns `None` on missing entry or
/// encoding errors.
fn read_zip_entry_str(bytes: &[u8], name: &str) -> Option<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

// ── tests ──────────────────────────────────────────────────────────────

/// Embedding a font must produce a `ppt/fonts/font1.fntdata` entry in the
/// resulting zip.
#[test]
fn embed_font_adds_fntdata_entry() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    assert!(
        zip_has_entry(&bytes, "ppt/fonts/font1.fntdata"),
        "ppt/fonts/font1.fntdata must exist after embed_font"
    );
}

/// `[Content_Types].xml` must gain an `<Override>` for the font part with the
/// correct `ContentType`.
#[test]
fn embed_font_adds_content_types_override() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    let ct = read_zip_entry_str(&bytes, "[Content_Types].xml")
        .expect("[Content_Types].xml must be present");
    assert!(
        ct.contains(r#"PartName="/ppt/fonts/font1.fntdata""#),
        "Override PartName missing; got:\n{ct}"
    );
    assert!(
        ct.contains("application/x-fontdata"),
        "Override ContentType missing; got:\n{ct}"
    );
}

/// `ppt/_rels/presentation.xml.rels` must reference the font entry with the
/// correct relationship type.
#[test]
fn embed_font_adds_presentation_rel() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    let rels = read_zip_entry_str(&bytes, "ppt/_rels/presentation.xml.rels")
        .expect("presentation.xml.rels must be present");
    assert!(
        rels.contains("fonts/font1.fntdata"),
        "rel Target missing; got:\n{rels}"
    );
    assert!(
        rels.contains("relationships/font"),
        "rel Type missing; got:\n{rels}"
    );
}

/// When the base PPTX has no `<p:embeddedFontLst>`, embedding a font must
/// create one containing the typeface entry and the variant element.
#[test]
fn embed_font_creates_embedded_font_lst_when_absent() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    let pres = read_zip_entry_str(&bytes, "ppt/presentation.xml")
        .expect("ppt/presentation.xml must be present");
    assert!(
        pres.contains("<p:embeddedFontLst>"),
        "<p:embeddedFontLst> must be created; got:\n{pres}"
    );
    assert!(
        pres.contains(r#"<p:font typeface="Pretendard""#),
        r#"<p:font typeface="Pretendard"> missing; got:\n{pres}"#
    );
    assert!(
        pres.contains("<p:regular r:id="),
        "<p:regular r:id=...> missing; got:\n{pres}"
    );
}

/// Embedding the same typeface twice with different variants must produce
/// exactly one `<p:embeddedFont>` block containing both variant elements.
#[test]
fn embed_font_merges_variants_into_one_block() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Bold, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    let pres = read_zip_entry_str(&bytes, "ppt/presentation.xml").unwrap();
    assert_eq!(
        pres.matches("<p:embeddedFont>").count(),
        1,
        "must be exactly 1 <p:embeddedFont> block; got:\n{pres}"
    );
    assert!(
        pres.contains("<p:regular"),
        "<p:regular> missing from merged block; got:\n{pres}"
    );
    assert!(
        pres.contains("<p:bold"),
        "<p:bold> missing from merged block; got:\n{pres}"
    );
}

/// Embedding two different typefaces must produce two separate `<p:embeddedFont>`
/// blocks.
#[test]
fn embed_font_keeps_typefaces_separate() {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    tmpl.embed_font("Pretendard", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    tmpl.embed_font("Inter", FontVariant::Regular, fake_ttf_bytes())
        .unwrap();
    let bytes = tmpl.pack().unwrap();
    let pres = read_zip_entry_str(&bytes, "ppt/presentation.xml").unwrap();
    assert_eq!(
        pres.matches("<p:embeddedFont>").count(),
        2,
        "must be 2 <p:embeddedFont> blocks (one per typeface); got:\n{pres}"
    );
}
