# canvas

Build AI slide editors in Rust.

Umbrella crate over the canvas-sdk workspace — bundles the pure data
model (`canvas-slide-doc`), the streaming LLM client
(`canvas-llm`), the generator + zone/section editors
(`canvas-editor`), and the editable PPTX writer
(`canvas-pptx-writer`) behind a single dependency and a feature-gated
public surface.

## Example

```rust,no_run
use std::sync::Arc;
use canvas::editor::CanvasGenerator;
use canvas::llm::FakeAiClient;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
let generator = CanvasGenerator::new(ai)?;
let _ = generator.build_prompt("표지 만들어줘", &[])?;
# Ok(())
# }
```

## Feature flags

- `slides` — `Deck`, `SlideDoc`, `SiteDoc` + HTML render (default)
- `llm` — `AiClient` + provider implementations (default)
- `editor` — generator + zone / section editors (default; implies
  `slides` + `llm`)
- `pptx-write` — PPTX export (default; implies `slides`)
- `test-util` — `canvas::llm::FakeAiClient` and editor test helpers
- `full` — alias for `slides` + `llm` + `editor` + `pptx-write`

Default features enable all four. Write-only consumers that only need
PPTX export can disable defaults:

```toml
canvas = { version = "0.2", default-features = false, features = ["pptx-write"] }
```
