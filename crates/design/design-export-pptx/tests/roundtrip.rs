//! Roundtrip verification via python-pptx. Skipped if python or python-pptx
//! isn't installed.

use kangnam_design_export_pptx::*;
use std::io::Write;
use std::process::Command;

fn python_pptx_available() -> bool {
    let status = Command::new("python3")
        .args(["-c", "import pptx"])
        .status();
    matches!(status, Ok(s) if s.success())
}

#[test]
fn python_pptx_reads_our_output() {
    if !python_pptx_available() {
        eprintln!("skipping: python3 + python-pptx not installed");
        return;
    }
    let deck = PptxDeck {
        title: Some("RT".into()),
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![
                PptxElement::Text(TextBox {
                    frame: Frame::from_px(100.0, 100.0, 400.0, 80.0),
                    content: "hello".into(),
                    style: TextStyle::default(),
                }),
                PptxElement::Shape(ShapeBox::new(
                    Frame::from_px(100.0, 300.0, 200.0, 100.0),
                    ShapeKind::Rect,
                    Fill::solid(Color(0x3B, 0x82, 0xF6)),
                    None,
                )),
            ],
        
        speaker_notes: None,
        }],
    };
    let bytes = write_deck_to_bytes(&deck).unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::File::create(tmp.path()).unwrap().write_all(&bytes).unwrap();

    let out = Command::new("python3")
        .args(["scripts/verify_pptx.py"])
        .arg(tmp.path())
        .output()
        .expect("python3 ran");
    assert!(out.status.success(), "python exit: {}\nstderr: {}",
            out.status, String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["slides"], 1);
    assert_eq!(v["text_boxes"], 1);
    assert_eq!(v["shapes"], 1);
    assert_eq!(v["images"], 0);
    assert_eq!(v["first_text"], "hello");
}
