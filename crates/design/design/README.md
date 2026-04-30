# design

kangnam-sdk design family umbrella crate.

Bundles the pure-data slide model (`design-doc-slide`), the site model
(`design-doc-site`), the streaming LLM client (`design-llm`), the
generic HTML zone editor (`design-editor-html`), the slide / site
generators (`design-editor-slide`, `design-editor-site`), and the
editable PPTX writer (`design-export-pptx`) behind a single dependency
and a feature-gated public surface.

Replaces the legacy `canvas` umbrella from canvas-sdk after the
2026-04-30 design-family rebrand.

## Example

```rust,no_run
use std::sync::Arc;
use design::editor_slide::CanvasGenerator;
use design::llm::FakeAiClient;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let ai = Arc::new(FakeAiClient::new(Vec::<String>::new()));
let generator = CanvasGenerator::new(ai)?;
let _ = generator.build_prompt("표지 만들어줘", &[])?;
# Ok(())
# }
```

## Feature flags

- `slide` — `Deck`, `SlideDoc` + slide IR (default)
- `site` — `SiteDoc` + HTML render + manifest ingest + zone inject (default; implies `slide`)
- `llm` — `AiClient` + provider implementations (default)
- `editor-html` — generic HTML zone editor (default; implies `llm`)
- `editor-slide` — slide deck generator + section editor (default; implies `slide` + `llm`)
- `editor-site` — site (landing page) generator (default; implies `site` + `llm`)
- `pptx-write` — PPTX export (default; implies `slide`)
- `test-util` — `design::llm::FakeAiClient` and editor test helpers
- `full` — alias for all of the above

Default features enable everything. Write-only consumers that only need
PPTX export can disable defaults:

```toml
design = { version = "0.2", default-features = false, features = ["pptx-write"] }
```
