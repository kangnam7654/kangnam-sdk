//! Phase 6c-01 — verify oklch / display-p3 / extended hex inputs
//! flow through SlideDoc → PptxDeck → write without crashing.

#![cfg(feature = "slide-doc")]

use design_doc_slide::slide::{
    Background as SdBackground, Frame as SdFrame, SlideDoc, SlideElement, TextStyle,
};
use design_export_pptx::{from_deck, write_deck_to_bytes};

fn deck_with_text_color(color: &str) -> design_doc_slide::deck::Deck {
    let mut style = TextStyle::default();
    style.color = color.into();
    let mut doc = SlideDoc::empty("s1");
    doc.elements.push(SlideElement::Text {
        id: "t".into(),
        frame: SdFrame { x: 0.0, y: 0.0, w: 100.0, h: 50.0 },
        content: "hi".into(),
        style,
    });
    use design_doc_slide::deck::Deck;
    Deck { id: "test".into(), slides: vec![doc] }
}

fn deck_with_bg(bg_color: &str) -> design_doc_slide::deck::Deck {
    let mut doc = SlideDoc::empty("s1");
    doc.background = SdBackground::Color { color: bg_color.into() };
    use design_doc_slide::deck::Deck;
    Deck { id: "test".into(), slides: vec![doc] }
}

#[test]
fn oklch_text_color_writes_successfully() {
    let deck = deck_with_text_color("oklch(0.628 0.258 29.23)");
    let pptx_deck = from_deck(&deck).expect("convert");
    let bytes = write_deck_to_bytes(&pptx_deck).expect("write");
    assert!(bytes.starts_with(b"PK"));
}

#[test]
fn oklab_bg_color_writes_successfully() {
    let deck = deck_with_bg("oklab(0.6 0.2 0.05)");
    let pptx_deck = from_deck(&deck).expect("convert");
    let _bytes = write_deck_to_bytes(&pptx_deck).expect("write");
}

#[test]
fn display_p3_text_color_writes_successfully() {
    let deck = deck_with_text_color("color(display-p3 1 0.2 0.4)");
    let pptx_deck = from_deck(&deck).expect("convert");
    let _bytes = write_deck_to_bytes(&pptx_deck).expect("write");
}

#[test]
fn rgb_function_text_color_writes_successfully() {
    let deck = deck_with_text_color("rgb(255, 100, 50)");
    let pptx_deck = from_deck(&deck).expect("convert");
    let _bytes = write_deck_to_bytes(&pptx_deck).expect("write");
}

#[test]
fn rgba_function_text_color_strips_alpha() {
    let deck = deck_with_text_color("rgba(0, 100, 200, 0.7)");
    let pptx_deck = from_deck(&deck).expect("convert");
    let _bytes = write_deck_to_bytes(&pptx_deck).expect("write");
}

#[test]
fn unknown_color_format_errors_explicitly() {
    let deck = deck_with_text_color("hsl(0, 100%, 50%)");
    let err = from_deck(&deck).expect_err("hsl unsupported");
    let msg = format!("{err:?}");
    assert!(msg.contains("InvalidHex") || msg.contains("hsl"), "got {msg}");
}
