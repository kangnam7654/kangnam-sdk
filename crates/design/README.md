# canvas-sdk

Reusable Rust building blocks for the [Canvas](https://github.com/USER/design-canvas) local AI designer and anything else that wants to generate, edit, and export slide-like documents.

## Crates

| Crate | What it does | Status |
| :--- | :--- | :--- |
| [`canvas-slide-doc`](crates/canvas-slide-doc) | Slide/Deck/Site data model, HTML render, DOM-manifest ingest | planned v0.2 |
| [`canvas-llm`](crates/canvas-llm) | `AiClient` trait + Gemini/Claude/LM Studio providers | planned v0.2 |
| [`canvas-editor`](crates/canvas-editor) | Generator, zone editor, section editor, site generator | planned v0.2 |
| [`canvas-pptx-writer`](crates/canvas-pptx-writer) | Editable PPTX writer (pure Rust, no LibreOffice) | v0.1.0 |
| [`canvas`](crates/canvas) | Umbrella crate with feature flags | planned v0.2 |

## Usage

Most consumers only need the umbrella:

```toml
canvas = { path = "...", features = ["full"] }
```

Feature flags: `slides`, `llm`, `editor`, `pptx-write`, `full`, `test-util`.

## Workspace layout

Single `Cargo.lock` + shared `target/`. Run any command at the workspace root:

```sh
cargo build                # all crates
cargo test --workspace     # all tests
cargo test -p canvas-pptx-writer   # single crate
```

## License

MIT OR Apache-2.0.
