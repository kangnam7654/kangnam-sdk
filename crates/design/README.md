# kangnam-design family

Rust building blocks for AI-assisted design — slide / site / mobile / dashboard prototypes, SKILL.md catalogs, brand `DESIGN.md` systems, prompt composition, PPTX export, and a wire-compatible port of the open-design daemon ⇆ web contracts.

The family supersedes the legacy `canvas-*` crates and absorbs the open-design v0.4 catalog (skills, design systems, craft guides, prompt templates, deck scaffolds) into vendored `&'static str` constants and on-disk catalogs.

## Crates

| Crate | What it does |
| :--- | :--- |
| [`kangnam-design`](design) | Umbrella — re-exports every sister crate behind feature flags. Single dep for downstream consumers. |
| [`kangnam-design-doc-slide`](design-doc-slide) | Slide / deck IR (PPTX-shaped, JSON-serializable). |
| [`kangnam-design-doc-site`](design-doc-site) | Site (landing-page) IR with HTML render + DOM-manifest ingest. |
| [`kangnam-design-llm`](design-llm) | Streaming `AiClient` trait + Gemini CLI / Claude CLI / LM Studio providers. |
| [`kangnam-design-editor-html`](design-editor-html) | Generic HTML zone editor — raw-HTML zone walking + LLM-driven edits. |
| [`kangnam-design-editor-slide`](design-editor-slide) | LLM-driven slide deck generator + section editor. |
| [`kangnam-design-editor-site`](design-editor-site) | LLM-driven landing-page generator. |
| [`kangnam-design-export-pptx`](design-export-pptx) | Pure-Rust editable `.pptx` writer (no LibreOffice). |
| [`kangnam-design-direction`](design-direction) | 5 curated visual directions (Editorial, Modern Minimal, Tech Utility, Brutalist, Soft Warm) with deterministic OKLch palettes. |
| [`kangnam-design-system`](design-system) | DESIGN.md 9-section parser + token extractor — **139 vendored systems** (cursor, linear-app, stripe, vercel, agentic, shadcn, …). |
| [`kangnam-design-skill`](design-skill) | SKILL.md frontmatter parser with `od:` extension support — **64 vendored skills** (web-prototype, dashboard, html-ppt family of 16 themes, kami-deck, hatch-pet, guizang-ppt, …). Optional `craft` feature wires `DesignSkill::resolve_crafts()` to `kangnam-design-craft`. |
| [`kangnam-design-prompt`](design-prompt) | System-prompt composer — DISCOVERY directives + identity charter + active skill body + active DESIGN.md + project metadata. |
| [`kangnam-design-prompt-template`](design-prompt-template) | Catalog of **94 ready-made image / video gen prompts** (profile-avatar, social-media, hyperframes, seedance, …) with `TemplateFilter` extension trait for chainable filters. |
| [`kangnam-design-craft`](design-craft) | 7 brand-agnostic craft references (typography, color, anti-AI-slop, accessibility-baseline, animation-discipline, rtl-and-bidi, state-coverage). Skills opt in via `od.craft.requires`. |
| [`kangnam-design-html-template`](design-html-template) | 2 vendored HTML scaffolds (`DECK_FRAMEWORK`, `KAMI_DECK`) for deck-mode skills that don't ship their own seed template. |
| [`kangnam-design-contracts`](design-contracts) | Pure-Rust port of `@open-design/contracts` — wire-compatible boundary types: 11 REST modules, SSE protocol, critique theater, sidecar IPC. **150+ tests** covering camelCase / snake_case / kebab-case round-trips, double-Option for `T \| null \| undefined`, locked-string discriminants. |
| [`kangnam-design-artifact`](design-artifact) | Streaming `<artifact>` parser + `<question-form>` schema + P0/P1 lint + sandboxed srcdoc HTML wrapper. |
| [`kangnam-design-tools`](design-tools) | 8 design-mode tools for the harness runtime: ask, scaffold, skill, preview, tweaks, done, brand_asset_extract, gen_image. |

## Usage

Most consumers depend on the umbrella with the `full` feature:

```toml
[dependencies]
kangnam-design = { version = "0.3", features = ["full"] }
```

Or pick à la carte:

```toml
[dependencies]
kangnam-design = { version = "0.3", default-features = false, features = ["slide", "pptx-write", "skill"] }
```

Feature flags on the umbrella: `slide`, `site`, `llm`, `editor-html`, `editor-slide`, `editor-site`, `pptx-write`, `craft`, `prompt-template`, `contracts`, `html-template`, `skill`, `system`, `direction`, `prompt`, `artifact`, `full`, `test-util`.

The `craft` umbrella feature additionally propagates to `kangnam-design-skill?/craft` so `DesignSkill::resolve_crafts()` works without extra config when both are enabled (the default).

## Vendored data licensing

Vendored data carries upstream licenses preserved verbatim. Major attributions:

- Skills + design systems + craft guides + deck scaffolds: [open-design](https://github.com/nexu-io/open-design) — Apache-2.0.
- Bundled `guizang-ppt`, `html-ppt`, `hatch-pet` skills retain their original LICENSE files (MIT / Apache-2.0).
- Prompt templates: [open-design](https://github.com/nexu-io/open-design) — Apache-2.0; [YouMind-OpenLab/awesome-gpt-image-2](https://github.com/YouMind-OpenLab/awesome-gpt-image-2) — CC-BY-4.0; [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes) — Apache-2.0. Each template's `source` field carries the upstream repo, license, author, and URL.
- Craft markdown adapted from [refero_skill](https://github.com/referodesign/refero_skill) (MIT) via open-design.
- Protocol references (`docs/skills-protocol.md`, `docs/modes.md`) vendored from open-design (Apache-2.0).

The crate code is `LicenseRef-Proprietary-AllRightsReserved` (per workspace `Cargo.toml`), but the vendored data subdirectories (`skills/`, `systems/`, `crafts/`, `templates/`, `docs/`) honour their upstream licenses — clone, redistribute, or re-license per upstream terms.

## Workspace layout

Single `Cargo.lock` + shared `target/` at the workspace root. Run cargo from anywhere in the tree:

```sh
cargo build                                  # all crates
cargo test --workspace --tests               # all unit + integration tests
cargo test -p kangnam-design-contracts       # one crate
cargo test -p kangnam-design-skill --features craft  # feature-gated tests
```

## See also

- Workspace [`Cargo.toml`](../../Cargo.toml) — full crate list and shared dependency versions.
- Per-crate `Cargo.toml` — feature flags, optional deps, MSRV.
- [`CHANGELOG.md`](CHANGELOG.md) — version history (current: v0.3.5 with the `[Unreleased]` block listing all open-design absorption rounds).
