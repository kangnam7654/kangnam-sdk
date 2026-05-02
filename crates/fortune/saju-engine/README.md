# saju-engine

Korean four-pillars (사주팔자) astrology computation engine — pure Rust, no IO.

## Purpose

Library for computing Korean four-pillars astrology: given a birth date and time, returns typed values (`FourPillars`, `ElementBalance`, `TenGod`, …) plus Korean interpretation text for daily, monthly, and daeun (대운 10-year luck period) fortunes. Rule-based, no LLM, no network calls. All dates are KST (UTC+9).

## Installation

```toml
[dependencies]
saju-engine = { git = "https://github.com/kangnam7654/saju-engine", tag = "v0.1.0" }
```

## Quick Start

```rust
use saju_engine::{calculate_four_pillars, ElementBalance};

let pillars = calculate_four_pillars(1990, 5, 15, 14);
let balance = ElementBalance::from_pillars(&pillars);
let total = balance.wood + balance.fire + balance.earth + balance.metal + balance.water;
assert_eq!(total, 8);
```

## API Overview

- `SajuEngine::generate(reading_type, input) -> (Value, String)` — unified JSON entry point for `daily`, `daily_detail`, `weekly`, `monthly`, `saju`, `saju_full`, `compatibility`, `compatibility_detail`, `monthly_fortune`, `daeun`.
- `calculate_four_pillars(year, month, day, hour) -> FourPillars`
- `ElementBalance::from_pillars(pillars) -> ElementBalance`
- `types` module — `Stem`, `Branch`, `Element`, `Polarity`, `TenGod`, `Pillar`, `FourPillars`, `ElementBalance`.
- `pillars`, `elements`, `ten_gods`, `branches`, `interpreter`, `daily`, `monthly`, `daeun`, `tables` submodules — direct access for finer control.

## Examples

See `examples/`:

- `cargo run --example saju_minimal` — generates a daily reading for a sample birth date and prints the JSON envelope.

## Stability

v0.x — API may change between minor versions. v1.0 will commit to semver.

## License

MIT. See [LICENSE](LICENSE).
