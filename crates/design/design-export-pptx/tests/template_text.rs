//! Integration tests for v0.3.3 rich text attributes on PptxTemplate::add_element(Text).
//!
//! Each test appends a slide, calls add_element with a TextBox, packs the PPTX,
//! and asserts on the slide XML inside the zip archive.
//!
//! Note: `TextStyle` is `#[non_exhaustive]`, so struct literal construction from
//! this external test crate uses `Default::default()` + field mutation.

use std::io::{Cursor, Read};

use kangnam_design_export_pptx::*;

// ── helpers ────────────────────────────────────────────────────────────────

fn base_pptx_bytes() -> Vec<u8> {
    let deck = PptxDeck {
        title: Some("RichTextTests".into()),
        slides: vec![PptxSlide::blank_1280_720()],
    };
    write_deck_to_bytes(&deck).expect("base pptx")
}

fn read_slide_xml(bytes: &[u8], slide_num: usize) -> String {
    let name = format!("ppt/slides/slide{}.xml", slide_num);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let mut entry = archive.by_name(&name).expect(&name);
    let mut s = String::new();
    entry.read_to_string(&mut s).expect("read_to_string");
    s
}

fn add_text_slide(tb: TextBox) -> String {
    let mut tmpl = PptxTemplate::load_bytes(&base_pptx_bytes()).unwrap();
    let s = tmpl.add_slide_from_layout(1).unwrap();
    tmpl.add_element(s, PptxElement::Text(tb)).unwrap();
    let bytes = tmpl.pack().unwrap();
    // Base pptx already has slide1; appended slide is slide2.
    read_slide_xml(&bytes, 2)
}

// ── test 1: basic text emits <p:sp txBox="1"> ─────────────────────────────

#[test]
fn add_element_text_emits_sp_with_txbox() {
    let xml = add_text_slide(TextBox {
        frame: Frame::from_px(10.0, 10.0, 400.0, 80.0),
        content: "hello".into(),
        style: TextStyle::default(),
    });
    assert!(
        xml.contains(r#"<p:cNvSpPr txBox="1"/>"#),
        "expected <p:cNvSpPr txBox=\"1\"/> in:\n{xml}"
    );
    assert!(xml.contains("<p:sp>"), "expected <p:sp> in:\n{xml}");
    assert!(xml.contains("hello"), "expected text content in:\n{xml}");
}

// ── test 2: font_weight=700 emits b="1" ───────────────────────────────────

#[test]
fn text_with_bold_emits_b_attr() {
    let bold_xml = {
        let mut style = TextStyle::default();
        style.font_weight = 700;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "bold text".into(),
            style,
        })
    };
    assert!(
        bold_xml.contains(r#"b="1""#),
        "font_weight=700 must emit b=\"1\": {bold_xml}"
    );

    // font_weight=600 must NOT emit b="1" (threshold is >= 700)
    let normal_xml = {
        let mut style = TextStyle::default();
        style.font_weight = 600;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "normal text".into(),
            style,
        })
    };
    assert!(
        !normal_xml.contains(r#"b="1""#),
        "font_weight=600 must NOT emit b=\"1\": {normal_xml}"
    );
}

// ── test 3: italic=true emits i="1" ───────────────────────────────────────

#[test]
fn text_with_italic_emits_i_attr() {
    let italic_xml = {
        let mut style = TextStyle::default();
        style.italic = true;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "italic text".into(),
            style,
        })
    };
    assert!(
        italic_xml.contains(r#"i="1""#),
        "italic=true must emit i=\"1\": {italic_xml}"
    );

    let upright_xml = {
        let mut style = TextStyle::default();
        style.italic = false;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "upright text".into(),
            style,
        })
    };
    assert!(
        !upright_xml.contains(r#"i="1""#),
        "italic=false must NOT emit i=\"1\": {upright_xml}"
    );
}

// ── test 4: color_alpha emits <a:alpha val="50000"/> ──────────────────────

#[test]
fn text_with_alpha_emits_alpha_tag() {
    let alpha_xml = {
        let mut style = TextStyle::default();
        style.color = Color(0xFF, 0x00, 0x00);
        style.color_alpha = Some(50_000);
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "alpha text".into(),
            style,
        })
    };
    assert!(
        alpha_xml.contains(r#"<a:alpha val="50000"/>"#),
        "color_alpha=Some(50000) must emit <a:alpha val=\"50000\"/>: {alpha_xml}"
    );

    let opaque_xml = {
        let mut style = TextStyle::default();
        style.color_alpha = None;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "opaque text".into(),
            style,
        })
    };
    assert!(
        !opaque_xml.contains("<a:alpha"),
        "color_alpha=None must not emit <a:alpha>: {opaque_xml}"
    );
}

// ── test 5: letter_spacing_pt=1.5 emits spc="150" ────────────────────────

#[test]
fn text_letter_spacing_emits_spc_attr() {
    let spc_xml = {
        let mut style = TextStyle::default();
        style.letter_spacing_pt = 1.5;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "spaced text".into(),
            style,
        })
    };
    assert!(
        spc_xml.contains(r#"spc="150""#),
        "letter_spacing_pt=1.5 must emit spc=\"150\": {spc_xml}"
    );

    let no_spc_xml = {
        let mut style = TextStyle::default();
        style.letter_spacing_pt = 0.0;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 60.0),
            content: "no spacing".into(),
            style,
        })
    };
    assert!(
        !no_spc_xml.contains("spc="),
        "letter_spacing_pt=0 must not emit spc attr: {no_spc_xml}"
    );
}

// ── test 6: line_height=0.82 emits <a:spcPct val="82000"/> ───────────────

#[test]
#[allow(non_snake_case)]
fn text_line_height_emits_lnSpc() {
    let lh_xml = {
        let mut style = TextStyle::default();
        style.line_height = 0.82;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 80.0),
            content: "tight leading".into(),
            style,
        })
    };
    assert!(
        lh_xml.contains("<a:lnSpc>"),
        "line_height=0.82 must emit <a:lnSpc>: {lh_xml}"
    );
    assert!(
        lh_xml.contains(r#"val="82000""#),
        "line_height=0.82 must emit val=\"82000\" (×100_000): {lh_xml}"
    );

    let zero_lh_xml = {
        let mut style = TextStyle::default();
        style.line_height = 0.0;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 300.0, 80.0),
            content: "zero lh".into(),
            style,
        })
    };
    assert!(
        !zero_lh_xml.contains("<a:lnSpc>"),
        "line_height=0.0 must not emit <a:lnSpc>: {zero_lh_xml}"
    );
}

// ── test 7: allow_wrap=false emits wrap="none" ────────────────────────────

#[test]
fn text_allow_wrap_false_emits_wrap_none() {
    let nowrap_xml = {
        let mut style = TextStyle::default();
        style.allow_wrap = false;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 200.0, 40.0),
            content: "no wrap".into(),
            style,
        })
    };
    assert!(
        nowrap_xml.contains(r#"wrap="none""#),
        "allow_wrap=false must emit wrap=\"none\": {nowrap_xml}"
    );

    let wrap_xml = {
        let mut style = TextStyle::default();
        style.allow_wrap = true;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 200.0, 40.0),
            content: "wraps".into(),
            style,
        })
    };
    assert!(
        wrap_xml.contains(r#"wrap="square""#),
        "allow_wrap=true must emit wrap=\"square\": {wrap_xml}"
    );
}

// ── test 8: newlines produce multiple <a:p> ───────────────────────────────

#[test]
fn text_with_newlines_produces_multiple_paragraphs() {
    let xml = add_text_slide(TextBox {
        frame: Frame::from_px(0.0, 0.0, 400.0, 200.0),
        content: "a\nb\nc".into(),
        style: TextStyle::default(),
    });
    let count = xml.matches("<a:p>").count();
    assert_eq!(
        count, 3,
        "\"a\\nb\\nc\" must produce 3 <a:p> paragraphs, got {count}: {xml}"
    );
    assert!(xml.contains(">a<"), "line a must appear: {xml}");
    assert!(xml.contains(">b<"), "line b must appear: {xml}");
    assert!(xml.contains(">c<"), "line c must appear: {xml}");
}

// ── test 9: TextAlign variants emit correct algn attr ────────────────────

#[test]
fn text_align_center_emits_ctr() {
    let center_xml = {
        let mut style = TextStyle::default();
        style.align = TextAlign::Center;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 400.0, 60.0),
            content: "centered".into(),
            style,
        })
    };
    assert!(
        center_xml.contains(r#"algn="ctr""#),
        "TextAlign::Center must emit algn=\"ctr\": {center_xml}"
    );

    let left_xml = {
        let mut style = TextStyle::default();
        style.align = TextAlign::Left;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 400.0, 60.0),
            content: "left".into(),
            style,
        })
    };
    assert!(
        left_xml.contains(r#"algn="l""#),
        "TextAlign::Left must emit algn=\"l\": {left_xml}"
    );

    let right_xml = {
        let mut style = TextStyle::default();
        style.align = TextAlign::Right;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 400.0, 60.0),
            content: "right".into(),
            style,
        })
    };
    assert!(
        right_xml.contains(r#"algn="r""#),
        "TextAlign::Right must emit algn=\"r\": {right_xml}"
    );

    let just_xml = {
        let mut style = TextStyle::default();
        style.align = TextAlign::Justify;
        add_text_slide(TextBox {
            frame: Frame::from_px(0.0, 0.0, 400.0, 60.0),
            content: "justified".into(),
            style,
        })
    };
    assert!(
        just_xml.contains(r#"algn="just""#),
        "TextAlign::Justify must emit algn=\"just\": {just_xml}"
    );
}
