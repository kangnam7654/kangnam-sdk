# saju-engine

Korean four-pillars (사주팔자) astrology computation engine — pure Rust, no IO.

## Purpose

Library for computing Korean four-pillars astrology: given a birth date and time, returns typed values (`FourPillars`, `ElementBalance`, `TenGod`, …) and calculation-only JSON facts for daily, monthly, and daeun (대운 10-year luck period) readings. Rule-based, no LLM, no network calls. All dates are KST (UTC+9). User-facing interpretation belongs in the consuming backend/service layer.

## Calculation Profile

The current product profile is locked as
`lunar_6tail_compatible_kr_service` / `v1`:

- **Compatibility target**: `6tail-lunar-compatible`
- **Primary calculation reference**:
  `6tail/lunar-python` and `6tail/lunar-javascript` EightChar/BaZi
  contracts
- **Calendar policy**: Korean lunar calendar via `rs-klc`, aligned with
  Dalgyeol/KST and the ziwei engine. Chinese-calendar reference engines can
  differ in rare leap-month years, and those cases must be marked by fixture.
- **Sect policy**: civil-date Zi hour; no late-Zi day shift unless a future
  profile explicitly opts into that school.
- **Unsupported policy**: any rule without fixture coverage is emitted as
  approximate or pending instead of authoritative.

The regression suite includes open-source compatibility fixtures:

- `6tail/lunar-python` README: lunar `1986-04-21 00:00` converts to solar
  `1986-05-29` and yields `병인/계사/계유/임자`.
- `6tail/lunar-javascript` EightChar docs: solar `2005-12-23 08:37`
  yields `을유/무자/신사/임진`, with hidden stems locked for all four
  branches.
- Additional `6tail/lunar-javascript` fixtures cover solar `1988-02-15
  22:30`, solar `1988-02-02 22:30`, lunar `2019-12-12 11:22`, and solar
  `1999-06-07 09:11`.

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

- `generate_saju_reading(SajuEngineRequest) -> Result<SajuEngineResponse, SajuEngineError>` — strict high-level SDK API, shaped to match tarot-engine's `generate_tarot_reading`.
- `generate_daily_saju`, `generate_saju_profile`, `generate_saju_compatibility` — convenience wrappers over the strict API.
- `parse_birth_input`, `parse_birth_time` — shared date/time validation helpers for app code.
- `SAJU_ENGINE_VERSION`, `SAJU_READING_TYPES`, `SAJU_DAILY_CATEGORIES`, `is_valid_reading_type` — public constants/helpers for routing, caching, and UI policy.
- `SajuEngine::generate(reading_type, input) -> (Value, String)` — unified JSON entry point for `daily`, `daily_detail`, `weekly`, `monthly`, `saju`, `compatibility`, `compatibility_detail`, `monthly_fortune`, `daeun`.
- `calculate_four_pillars(year, month, day, hour) -> FourPillars`
- `ElementBalance::from_pillars(pillars) -> ElementBalance`
- `types` module — `Stem`, `Branch`, `Element`, `Polarity`, `TenGod`, `Pillar`, `FourPillars`, `ElementBalance`.
- `pillars`, `elements`, `ten_gods`, `branches`, `daily`, `monthly`, `daeun`, `tables` submodules — direct access for finer control.

```rust
use saju_engine::{generate_saju_reading, SajuEngineRequest};

let response = generate_saju_reading(SajuEngineRequest {
    reading_type: "saju",
    birth_date: Some("1990-05-15"),
    birth_time: Some("14:30"),
    calendar_type: Some("solar"),
    gender: Some("M"),
    ..Default::default()
})?;

assert_eq!(response.engine_version, "saju-v1.0");
assert!(response.result_json["four_pillars"].is_object());
```

## Examples

See `examples/`:

- `cargo run --example saju_minimal` — generates a daily reading for a sample birth date and prints the JSON envelope.

## Stability

v0.x — API may change between minor versions. v1.0 will commit to semver.

## License

MIT. See [LICENSE](LICENSE).
