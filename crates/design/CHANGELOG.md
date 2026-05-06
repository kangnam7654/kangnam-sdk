# Changelog

## [Unreleased]

### Added
- **New crate `kangnam-design-html-template`** — vendored HTML scaffold templates for deck-mode skills (`DECK_FRAMEWORK`, `KAMI_DECK`). Embedded as `&'static str` via `include_str!`. Public API: `HtmlTemplate` struct (id, title, when_to_use, body), `TEMPLATES` slice, `template_by_id(id)` lookup. 5 unit tests + 1 doctest. Re-exported from `kangnam-design` umbrella under the new `html-template` feature flag (in default set). Counterpart to `kangnam-design-prompt` (system prompts) and `kangnam-design-prompt-template` (gen-media prompts).
- **New crate `kangnam-design-contracts`** — pure-Rust port of open-design's `@open-design/contracts` package. Wire-compatible with the upstream TypeScript zod schemas (camelCase serialization, lowercase enum variants, optional fields skip-on-`None`). Modules: `common` (`OkResponse`, `IdResponse`, `BoundedJsonConstraints`, `LIVE_ARTIFACT_BOUNDED_JSON_CONSTRAINTS`), `errors` (42-variant `ApiErrorCode` enum with strict deserialization, `ApiError` envelope, `ApiValidationIssue`, `SseErrorPayload`, `create_api_error`/`create_api_error_response` helpers), `tasks` (6-state `TaskState` enum with `is_terminal()` guard, `TaskStatus` snapshot, `TASK_STATES` slice), `sse` (generic `SseEvent<P>` envelope, discriminated `DaemonAgentPayload` covering text-delta / thinking / tool-use / tool-result / live-artifact / usage / raw, `ChatSseEvent` + `ProxySseEvent` event-name discriminators, `CHAT_SSE_PROTOCOL_VERSION` / `PROXY_SSE_PROTOCOL_VERSION`). 34 unit tests. Re-exported from `kangnam-design` umbrella under the new `contracts` feature flag (in default set). API endpoints (`api/*`), `critique.ts`, and `examples.ts` remain TODO.
- **New crate `kangnam-design-prompt-template`** — catalog of ready-made image / video generation prompt templates. Vendors **94 templates** from open-design v0.4 (44 image: profile-avatar, social-media, illustration, infographic, e-commerce, …; 50 video: cinematic, hyperframes, seedance, K-pop dance, retro, …) with their original schema (`id`, `surface`, `title`, `summary`, `category`, `tags`, `model`, `aspect`, `prompt`, `previewImageUrl`, `previewVideoUrl`, `source: {repo, license, author, url}`). Public API: typed `PromptTemplate` + `Surface` enum (`#[non_exhaustive]`), `load_templates_from_dir` / `load_templates_from_surface_dir` / `list_template_ids`, `has_tag` / `in_category` filter helpers, JSON-path-aware error reporting, round-trip preservation of unknown fields. 12 unit tests + 1 doctest. Distinct from `kangnam-design-prompt` (system-prompt composer). Re-exported from `kangnam-design` umbrella under the new `prompt-template` feature flag (in default set).
- **New crate `kangnam-design-craft`** — brand-agnostic craft references. Vendors the open-design v0.4 `craft/` directory (typography, color, anti-AI-slop, accessibility-baseline, animation-discipline, rtl-and-bidi, state-coverage) as `&'static str` constants via `include_str!`, plus a runtime loader for user-supplied craft files. Public API:
  - `Craft` (zero-alloc static record) + `OwnedCraft` (heap-loaded variant).
  - 7 built-in constants (`TYPOGRAPHY`, `COLOR`, `ANTI_AI_SLOP`, `STATE_COVERAGE`, `ANIMATION_DISCIPLINE`, `ACCESSIBILITY_BASELINE`, `RTL_AND_BIDI`) + `BUILTIN_CRAFTS` slice.
  - `craft_by_id(id)` — single-slug lookup.
  - `requires_to_crafts(slugs)` — resolve a `od.craft.requires` list, preserving order, dedupe, drop unknowns silently (forward-compat).
  - `render_for_prompt(crafts)` — concatenate into one system-prompt block (`## <title>` per section).
  - `load_crafts_from_dir(path)` + `list_craft_ids(path)` — disk loader for project-vendored crafts.
  - `AsCraftRef` polymorphic adapter so `Craft`, `OwnedCraft`, and `CraftRef` mix in one render call.
  - 17 unit tests + 2 doctests; covers letter-spacing assertion against vendored typography body.
  - Adapted from MIT-licensed [refero_skill](https://github.com/referodesign/refero_skill) via open-design (Apache-2.0) — both attributions preserved in crate docs.
- `kangnam-design-skill::OdMetadata.craft: OdCraft { requires: Vec<String> }` — typed parser for skills' `od.craft.requires` block (was previously captured anonymously in the `extras` flatten map). Resolution via `kangnam_design_craft::requires_to_crafts(&skill.od.craft.requires)` is intentionally left to the caller — design-skill stays craft-agnostic to avoid cross-crate coupling.
- `kangnam-design`: new `craft` feature flag (in default set) — re-exports `kangnam-design-craft` as `design::craft`.
- `kangnam-design-skill`: vendored catalog grew from **30 → 64 skills** by absorbing the open-design v0.4 catalog. New: `audio-jingle`, `design-brief`, `hatch-pet`, the entire `html-ppt` family (16 themes — pitch-deck, course-module, weekly-report, taste-brutalist/editorial, xhs-pastel-card/post/white-editorial, hermes-cyber-terminal, graphify-dark-graph, knowledge-arch-blueprint, obsidian-claude-gradient, presenter-mode-reveal, product-launch, tech-sharing, testing-safety-alert, dir-key-nav-minimal), `hyperframes`, `image-poster`, `kami-deck`, `kami-landing`, `live-artifact`, `open-design-landing`, `open-design-landing-deck`, `pptx-html-fidelity-audit`, `replit-deck`, `video-shortform`, `web-prototype-taste-brutalist`, `web-prototype-taste-editorial`, `web-prototype-taste-soft`. Each skill retains its bundled LICENSE file where present (e.g. `html-ppt`, `hatch-pet`, `guizang-ppt`).
- `kangnam-design-system`: vendored catalog grew from **73 → 139 systems** by absorbing the open-design v0.4 catalog. New themed systems include `agentic`, `ant`, `arc`, `atelier-zero`, `bento`, `brutalism`, `canva`, `claymorphism`, `cosmic`, `discord`, `dithered`, `doodle`, `duolingo`, `editorial`, `enterprise`, `fantasy`, `flat`, `friendly`, `futuristic`, `github`, `glassmorphism`, `gradient`, `huggingface`, `kami`, `levels`, `lingo`, `luxury`, `material`, `minimal`, `modern`, `mono`, `neobrutalism`, `neon`, `neumorphism`, `openai`, `pacman`, `paper`, `perspective`, `premium`, `professional`, `publication`, `refined`, `retro`, `shadcn`, `simple`, `skeumorphism`, `sleek`, `spacious`, `storytelling`, `tetris`, `vibrant`, `vintage`, plus modifier systems (`application`, `dashboard`, etc.).

### Changed
- Loader test floors raised: skills `>= 60` (was 25), systems `>= 130` (was 60). Canonical-id sanity checks extended (`html-ppt`, `hatch-pet`, `kami-deck` for skills; `agentic`, `shadcn`, `discord` for systems).

## [0.3.5] — 2026-04-28

### Fixed
- **Bug fix (visual change)**: `Background::Gradient.angle_deg` and `Fill::LinearGradient.angle_deg` now use the **same CSS convention** (0° = up, clockwise). Previously `Background::Gradient` was multiplied raw by 60_000 — meaning a SlideDoc passing `angle_deg: 90` (CSS = "to right") rendered as OOXML 90° = "down" in the exported PPTX, silently rotating gradients 90°. The `from_slide_doc` bridge was already passing CSS-degree values into the writer, so this was a cross-crate correctness issue. Consumers who hand-crafted `PptxDeck` with `Background::Gradient` and were compensating for the bug will see a 90° rotation in their output — pass `(old_deg + 90) % 360` to recover the previous direction.
- **Bug fix (visual change)**: `template::xml_ops::prst_geom_xml` for `RoundedRect` now correctly emits `adj = (radius_emu × 100_000) / min(w,h)` instead of `(radius_emu / min) × 50_000`. The previous formula produced corner radii **half** the requested size; templates editing rounded rects via `add_element` will now show the actual radius the caller asked for. The write-only `writer/shape.rs` path was already correct; this fix brings the template path into agreement.
- **Bug fix**: `ShapeBox::shadow` is now correctly emitted by the write-only `write_deck_to_bytes` path (previously silently dropped). Both write paths now emit byte-identical `<a:effectLst>` via the shared `OuterShadow::to_ooxml_effect_xml` helper.

### Changed
- `OuterShadow` gains `to_ooxml_effect_xml()` (`pub(crate)`) — the canonical XML emission helper, shared by the write-only and template-edit paths. Replaces `template::xml_ops::outer_shdw_xml` (now removed; was `pub(super)` so this is not a public API change).
- `geometry::roundrect_adj(radius_emu, w_emu, h_emu) -> i64` — new public helper consolidating the OOXML `roundRect` adj formula. Both `writer/shape.rs::emit_geometry` and `template/xml_ops.rs::prst_geom_xml` route through it.
- `writer/shape.rs::emit_fill` body collapsed from ~40 lines into a 4-line delegation to `Fill::to_ooxml_fill_xml()` (with a `TilePattern → noFill` special case for the write-only path). Eliminates the only place where a `Fill` change had to be made twice.
- `gs_list_xml` now `debug_assert!`s ascending stop position order — matches the existing doc-comment claim on `GradientStop`. Release builds still skip this check.
- `template::PptxTemplate::add_element` doc comment refreshed: no longer claims `Shape` returns an error (it has been fully implemented since v0.3.4).
- `upsert_slide_text_sp` error messages for missing `<p:txBody>` / `</p:txBody>` now include the placeholder idx so callers can map a failure back to the offending shape.

## [0.3.4] — 2026-04-25

### Added
- `OuterShadow` — new public type for outer drop shadow effects, mapped to OOXML `<a:effectLst><a:outerShdw>`. Fields: `dx_px`, `dy_px`, `blur_px` (all CSS-style pixels at 96 DPI), `color`, `alpha: Option<u32>` (per-mille, 0..=100_000). Re-exported from crate root.
- `ShapeBox::shadow: Option<OuterShadow>` — new field. `None` emits no `<a:effectLst>`. Shadow direction math: `dist = √(dx²+dy²) × 9525`, `dir = atan2(dy,dx) deg mod 360 × 60000` (matches `build_outer_shadow_xml` in dear-jeongbin `export_pptx_ooxml.rs`).
- `ShapeBox::new(frame, shape, fill, stroke) -> Self` — constructor that sets `shadow: None`. Required for external-crate construction now that `ShapeBox` is `#[non_exhaustive]`.
- `PptxTemplate::add_element(PptxElement::Shape(ShapeBox))` — fully implemented (previously returned `Err(Xml("scheduled for v0.3.4"))`). Handles all five fill variants: `Solid`, `LinearGradient`, `RadialGradient`, `TilePattern` (PNG embed + slide rels), and `None`. Also handles `Stroke` and `OuterShadow`. `Line` shape kind emits `prstGeom prst="line"` with forced `<a:noFill/>` (see TODO in code for potential `<p:cxnSp>` round-trip concern).
- 7 integration tests in `tests/template_shape.rs`: rect/rounded-rect/solid/gradient/tile/shadow + shadow direction math.

### Changed
- `#[non_exhaustive]` attribute added to `ShapeBox`. External-crate struct literals are now a compile error — use `ShapeBox::new(...)` + field mutation pattern.
- `PptxTemplate::add_tile_pattern_rect` marked `#[deprecated(since = "0.3.4")]`. Will be removed in v0.4.0. Use `add_element(PptxElement::Shape(ShapeBox { fill: Fill::TilePattern{..}, .. }))` instead.
- `Fill::to_ooxml_fill_xml` doc comment updated: no longer "transitional API scheduled for v0.3.5" — promoted to stable public API. Comments referencing "v0.3.5" corrected to "v0.3.4".

### Breaking changes
- `ShapeBox` has a new `shadow: Option<OuterShadow>` field. **Migration**: switch struct literals to `ShapeBox::new(frame, shape, fill, stroke)` (sets `shadow: None`), then assign `sb.shadow = Some(...)` if needed.
- `#[non_exhaustive]` on `ShapeBox` blocks external-crate struct literal construction (including `..Default::default()` spread). Use `ShapeBox::new`.
- `PptxTemplate::add_element(PptxElement::Shape(...))` no longer returns `Err` — it now succeeds. Callers that `unwrap_err()`'d this call will panic.

### Migration guide
```rust
// Before (any version — struct literal)
ShapeBox { frame, shape, fill, stroke: None }

// After (v0.3.4 — from any crate)
ShapeBox::new(frame, shape, fill, None)

// After (v0.3.4 — with shadow)
{
    let mut sb = ShapeBox::new(frame, shape, fill, None);
    sb.shadow = Some(OuterShadow { dx_px: 4.0, dy_px: 4.0, blur_px: 12.0, color: Color::BLACK, alpha: Some(40_000) });
    sb
}

// Before (add_tile_pattern_rect)
tmpl.add_tile_pattern_rect(slide, frame, &png_bytes, 0)?;

// After (v0.3.4 — add_element)
tmpl.add_element(slide, PptxElement::Shape(ShapeBox::new(frame, ShapeKind::Rect, Fill::TilePattern { png_bytes, tile_w_px: 24, tile_h_px: 24 }, None)))?;
```

## [0.3.3] — 2026-04-25

### Added
- `TextStyle` gains 3 new fields:
  - `italic: bool` (default `false`) → `<a:rPr i="1"/>` when `true`. Independent of `font_weight`.
  - `color_alpha: Option<u32>` (default `None`) — per-mille OOXML alpha (0..=100_000). `Some(50_000)` → `<a:srgbClr><a:alpha val="50000"/></a:srgbClr>`. `None` emits no `<a:alpha>` tag (fully opaque).
  - `allow_wrap: bool` (default `true`) → `<a:bodyPr wrap="square">` when `true`, `wrap="none"` when `false`.
- `#[non_exhaustive]` attribute added to `TextStyle` for forward compatibility with future field additions. External-crate consumers must use `Default::default()` + field mutation (struct literal syntax is no longer permitted outside the crate).
- `PptxTemplate::add_element(PptxElement::Text(TextBox))` is now fully implemented (previously returned `Err(Xml("scheduled for v0.3.3"))`). Emits a freeform `<p:sp txBox="1">` at absolute EMU coordinates with all `TextStyle` attrs applied.
- 9 integration tests in `tests/template_text.rs` covering all new attrs + alignment + newlines.

### Fixed
- **Bug fix (visual change)**: `TextStyle::line_height` was silently ignored by the v0.2 `write_deck` path (`writer/text.rs`). It is now correctly emitted as `<a:pPr><a:lnSpc><a:spcPct val="N"/></a:lnSpc></a:pPr>`. Conversion: `(line_height * 100_000).round() as u32` (CSS ratio → OOXML 1/1000-percent). Default `line_height = 1.2` → `val="120000"`. Consumers that relied on `line_height` being silently ignored will see a visual change in output PPTX — this is the intended behavior.

### Breaking changes
- `TextStyle` has 3 new required fields. **Migration**: add `..Default::default()` spread when constructing via struct literal (only valid within the same crate). External crates must use `Default::default()` + field mutation.
- `#[non_exhaustive]` on `TextStyle` means external-crate struct literals are now a compile error regardless of spread. Use `let mut s = TextStyle::default(); s.field = value; s` pattern.
- `PptxTemplate::add_element(PptxElement::Text(...))` no longer returns `Err` — it now succeeds. Callers that `unwrap_err()`'d this call will panic.

### Migration guide
```rust
// Before (v0.3.2, within same crate only)
TextStyle { font_family: "Pretendard".into(), font_size_pt: 24.0, ..Default::default() }

// After (v0.3.3, from any crate — including this crate's own tests)
{
    let mut s = TextStyle::default();
    s.font_family = "Pretendard".into();
    s.font_size_pt = 24.0;
    s
}
```

## [0.3.2] — 2026-04-25

### Added
- `canvas-pptx-writer`: `Fill` enum extended with three new variants:
  - `Fill::LinearGradient { angle_deg, stops: Vec<GradientStop> }` — multi-stop linear gradient. `angle_deg` follows CSS convention (0° = up, clockwise); converted to OOXML 1/60000-degree units internally.
  - `Fill::RadialGradient { stops: Vec<GradientStop> }` — multi-stop radial gradient (`<a:path path="circle">` centered at 50%/50%).
  - `Fill::TilePattern { png_bytes, tile_w_px, tile_h_px }` — 1:1 PNG tile fill, top-left aligned, embedded as `ppt/media/imageN.png`.
- New public type `GradientStop { position: f32, color: Color, alpha: Option<u32> }`. Re-exported from crate root.
- `Fill::dot_tile(tile_w_px, tile_h_px, dot_radius_px, color_rgba) -> Result<Fill>` — builds a `TilePattern` with an anti-aliased centered dot. Uses 2× supersampling + 2×2 box downsample (reduces LibreOffice bilinear brightening).
- `Fill::solid(color) -> Fill` — convenience constructor for fully-opaque solid fill (preferred over struct literal to avoid specifying the new `alpha` field).
- `Fill::to_ooxml_fill_xml(&self) -> String` — transitional public method that emits the OOXML fill fragment. Returns empty string for `TilePattern` (which requires slide-context zip mutation). Will be superseded by `add_element(Shape)` in v0.3.5.
- `PptxTemplate::add_tile_pattern_rect(slide, frame, png_bytes, border_radius_emu)` — transitional method that embeds a PNG tile and emits a `<p:sp>` with `<a:blipFill><a:tile/>` into the slide. Handles media embed, `[Content_Types].xml`, and slide rels automatically.
- `#[non_exhaustive]` attribute on `Fill` enum (forward-compatible for future variants).
- `image = "0.25"` (png feature only) added as default dependency for `Fill::dot_tile`.
- 8 integration tests in `tests/template_fills.rs` + 5 unit tests in `color.rs`.

### Changed
- `Fill::Gradient { from, to, angle_deg }` marked `#[deprecated(since = "0.3.2")]`. Kept for v0.2/v0.3.0 consumers; will be removed in v0.4.0. New code should use `Fill::LinearGradient`.
- `writer/shape.rs` and `from_slide_doc.rs` updated to handle new `Fill` variants (`TilePattern` falls back to `<a:noFill/>` in the write-only PptxDeck path since it requires slide context).

### Breaking changes
- `Fill::Solid { color }` → `Fill::Solid { color, alpha: Option<u32> }`. Any code constructing `Fill::Solid { color: c }` as a struct literal will fail to compile. **Migration**: use `Fill::solid(c)` or `Fill::Solid { color: c, alpha: None }`.
- `#[non_exhaustive]` on `Fill` means exhaustive match patterns (`_ => ...` wildcard not required in v0.3.1) now require a wildcard arm if all existing variants were matched. This affects code that matched all v0.3.1 variants without a wildcard.

### Migration guide
```rust
// Before (v0.3.1)
let fill = Fill::Solid { color: my_color };

// After (v0.3.2) — option A (preferred)
let fill = Fill::solid(my_color);

// After (v0.3.2) — option B (explicit)
let fill = Fill::Solid { color: my_color, alpha: None };
```

## [0.3.1] — 2026-04-25

### Added
- `canvas-pptx-writer`: `PptxTemplate::embed_font(typeface, variant, ttf_bytes)` for embedding custom TTF/OTF fonts (e.g. Pretendard) directly into a PPTX package.
  - New public type: `FontVariant` enum (`Regular`, `Bold`, `Italic`, `BoldItalic`). Re-exported from the crate root.
  - Mutates 4 in-memory entries on each call: appends `ppt/fonts/fontN.fntdata`, adds `<Override>` in `[Content_Types].xml`, adds `<Relationship Type=".../font">` in `ppt/_rels/presentation.xml.rels`, and upserts `<p:embeddedFontLst>` in `ppt/presentation.xml`.
  - Multi-variant calls for the same typeface merge into one `<p:embeddedFont>` block (matching PowerPoint authoring behavior).
  - Pre-existing embedded fonts loaded from the template are counted at load time; newly embedded fonts get unique `fontN` IDs.
- 6 integration tests in `tests/template_font_embed.rs`.

### Note on font licensing
Embedding a TTF/OTF redistributes the font binary inside the resulting `.pptx`. This library does **not** validate font licenses; consumers are responsible for ensuring the typeface permits redistribution (Pretendard: SIL OFL ✅, Apple System Fonts: ❌).

## [0.3.0] — 2026-04-25

### Added
- `canvas-pptx-writer`: `PptxTemplate` template-editing path that loads an existing `.pptx` (with slideMaster, slideLayouts, theme, embedded fonts), appends slides inheriting from layouts, fills `<p:ph idx="N"/>` placeholders, and re-zips. Counterpart to the write-only `PptxDeck` IR.
  - New types: `PptxTemplate`, `SlideRef`.
  - New methods: `load`, `load_bytes`, `layout_count`, `slide_size_emu`, `add_slide_from_layout`, `set_placeholder_text`, `set_placeholder_image`, `add_element` (Image variant; Text/Shape stubbed for v0.3.2-v0.3.4), `add_full_bleed_image`, `pack`, `write`.
  - Round-trip preserves untouched zip entries byte-identical (theme/master/layout). Layout/placeholder lookup errors are typed.
- `canvas-pptx-writer`: `PptxWriteError` gains `InvalidTemplate`, `LayoutNotFound`, `PlaceholderNotFound`, `SlideNotFound` variants and `#[non_exhaustive]`.
- 14 integration tests covering full lifecycle: load → add_slide → set_placeholder_text → set_placeholder_image → add_full_bleed_image → pack.

## [0.2.0] — 2026-04-24

### Added
- Workspace split into five crates: `canvas-slide-doc`, `canvas-llm`, `canvas-editor`, `canvas-pptx-writer`, and the umbrella `canvas`.
- `canvas-pptx-writer` gains `slide-doc` feature (default-on) that exposes `from_deck` / `from_slide_doc` helpers.
- `canvas-llm` gains `test-util` feature exposing `FakeAiClient`.

### Changed
- `canvas-pptx-writer` bumps from 0.1.0 → 0.2.0 (still backward-compatible for write-only consumers who disable `slide-doc`).

### Migration
- Direct consumers of `canvas-pptx-writer` v0.1: no change required (the old API is still there; the adapter is additive and opt-in).
- New integrations: add `canvas = { version = "0.2", features = ["full"] }` and import from `canvas::*` / `canvas::editor::*` / `canvas::pptx::*`.

## [0.1.0] — 2026-04-24

Initial release (canvas-pptx-writer only).
