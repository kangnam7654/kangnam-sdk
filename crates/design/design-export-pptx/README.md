# canvas-pptx-writer

Pure-Rust library that writes **editable** `.pptx` files (PowerPoint / Keynote / Google Slides) from a neutral slide-description data structure. No LibreOffice, no headless browser.

## Status

v0.1 — Level 1 features: text boxes, four shape kinds (rect / rounded-rect / ellipse / line), solid + gradient fills and strokes, PNG + JPEG image embedding, solid + gradient slide backgrounds, multi-slide decks.

## Example

```rust
use canvas_pptx_writer::*;
use std::path::Path;

fn main() -> Result<()> {
    let deck = PptxDeck {
        title: Some("Hello".into()),
        slides: vec![PptxSlide {
            width_emu: 12_192_000,
            height_emu: 6_858_000,
            background: Background::Solid { color: Color::WHITE },
            elements: vec![PptxElement::Text(TextBox {
                frame: Frame::from_px(100.0, 200.0, 1080.0, 200.0),
                content: "안녕, 캔버스".into(),
                style: TextStyle {
                    font_family: "Pretendard".into(),
                    font_size_pt: 48.0,
                    font_weight: 700,
                    color: Color(0x0E, 0xA5, 0xE9),
                    ..Default::default()
                },
            })],
        }],
    };
    write_deck(&deck, Path::new("out.pptx"))
}
```

## Units

All positions and sizes are EMU (914_400 per inch). Use `Frame::from_px(x,y,w,h)` at 96 DPI, or the `px_to_emu` / `pt_to_emu` helpers.

## Fonts

Font files are **not** embedded. Consumers must have the requested typeface installed; PowerPoint falls back to a default if missing.

## Out of scope (v0.1)

- Reading existing .pptx files (write-only).
- Tables, charts, SmartArt, animations.
- SVG images.
- Theme customization.
- Speaker notes.

See `docs/` in the Canvas repo for the design spec.

## License

MIT OR Apache-2.0.
