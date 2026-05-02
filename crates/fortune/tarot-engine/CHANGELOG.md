# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.1.0] - 2026-04-21

### Added
- Initial import from `lunawave`.
- Rider-Waite Major Arcana deck (22 cards) with per-card keywords, meanings, and element mapping.
- Spread drawing (`SpreadType::{OneCard, ThreeCard, CelticCross}`) with deterministic seeded draws.
- `TarotEngine::generate(reading_type, input)` — unified JSON entry point.
- Oriental five-element (오행) mapping via `Ohang` enum.

[Unreleased]: https://github.com/kangnam7654/tarot-engine/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kangnam7654/tarot-engine/releases/tag/v0.1.0
