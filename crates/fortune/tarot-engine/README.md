# tarot-engine

Rider-Waite-Smith tarot fortune engine — 78-card deck, spread drawing,
interpretation. Pure Rust, no IO.

## Purpose

Library for drawing tarot spreads (one-card / three-card / Celtic cross) from the Rider-Waite-Smith deck and generating per-card interpretation text. Rule-based, no LLM. Includes a Korean five-element (오행) mapping for apps that blend tarot with Korean fortune systems. Pure computation — no network, no database.

## Deck Source Profile

The current product profile is locked as
`rws_open_data_compatible_kr_service` / `v1`:

- **Compatibility target**:
  `rider-waite-smith-open-data-compatible`
- **Primary card identity reference**: open RWS card contracts such as
  `ekelen/tarot-api` and `metabismuth/tarot-json`
- **Default product draw pool**: major arcana 22 cards for backward
  compatibility
- **Full-deck mode**: `options.deck_scope = "full_78"` draws from all 78 cards
- **Unsupported policy**: non-RWS decks require a new profile ID and fixture
  set

## Installation

```toml
[dependencies]
tarot-engine = { git = "https://github.com/kangnam7654/tarot-engine", tag = "v0.1.0" }
```

## Quick Start

```rust
use tarot_engine::TarotEngine;
use serde_json::json;

let engine = TarotEngine;
let (reading, version) = engine.generate(
    "tarot_three",
    &json!({"birth_date": "1990-05-15"}),
);
println!("version={version}");
println!("{}", serde_json::to_string_pretty(&reading).unwrap());
```

## API Overview

- `TarotEngine::generate(reading_type, input) -> (Value, String)` — unified JSON entry point.
  - `reading_type`: `"tarot_daily"`, `"tarot_one"`, `"tarot_one_preview"`, `"tarot_three"`, `"tarot_celtic"`.
  - Unknown `reading_type` returns `({"error": "..."}, version)`.
- `TarotCard`, `DrawnCard`, `TarotReading` — typed response structs.
- `ArcanaType`, `Suit`, `TarotElement`, `Ohang`, `SpreadType` — enums for querying cards and spreads.
- `TAROT_ENGINE_VERSION` — compile-time version string for cache invalidation (currently `"tarot-v2.4"`).

## Examples

See `examples/`:

- `cargo run --example tarot_minimal` — draws a three-card spread and prints the JSON envelope.

## Stability

v0.x — API may change between minor versions. v1.0 will commit to semver.

## License

MIT. See [LICENSE](LICENSE).
