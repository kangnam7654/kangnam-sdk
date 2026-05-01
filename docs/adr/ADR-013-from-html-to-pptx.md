# ADR-013: HTML → PPTX strategy

- Status: **Accepted** (Phase 6d, 2026-05-01)
- Deciders: kangnam, design family contributors
- Module affected: [`crates/design/design-export-pptx/src/from_html.rs`](../../crates/design/design-export-pptx/src/from_html.rs)

## Context

Phase 6d (per `kangnam-chat-luminous-teacup.md`) targets parity with
huashu-design's `html2pptx.js`: take an HTML page (the kind a design
agent emits — Tailwind classes, CSS gradients, web fonts, scrollable
sections) and convert it into a faithful PPTX.

The huashu approach uses **Playwright + `pptxgenjs` + `sharp`**: it
boots a headless Chromium, renders the page, walks the DOM running
`getBoundingClientRect()` + `getComputedStyle()` over every element,
and translates the resulting box+style tree into PowerPoint shapes.

That's the only path to layout-faithful conversion. Pure-Rust HTML
parsers can extract the DOM but can't *resolve CSS* — they don't know
what an `<h1>` actually rendered as without a layout engine.

## Options considered

### (a) Sidecar Node + Playwright

Spawn a Node process from a Tauri command, hand it the HTML, get back
a serialized layout JSON the Rust writer can lower into `PptxDeck`.

- Pro: huashu parity unchanged — we'd run the same script.
- Pro: Rust side stays simple — just translation, no rendering.
- **Con: Node + Chromium dep on the user's machine** (≈ 200 MB).
- Con: cross-platform Playwright install is fragile.
- Con: extra process launch latency on every export.

### (b) Tauri webview hidden tab + JS bridge

Render the HTML into a hidden Tauri webview (already bundled with the
app), run a small layout-extraction JS over the DOM, ship the result
back to Rust via `__TAURI__.event.emit`. Rust then lowers into
`PptxDeck`.

- Pro: no extra binary deps — webview is already shipped.
- Pro: no process boundary, lower latency.
- Con: more bespoke code to write + maintain (the JS extraction
  script doesn't exist yet; we'd be porting `html2pptx.js`'s
  computed-style walk into webview-compatible JS).
- Con: hidden tab UX needs care (must not flash on screen).

### (c) Pure-Rust no-browser sketch

Parse HTML with regex / a lightweight HTML parser; extract just the
`<h1>/<h2>/<p>/<img>` content + the body's `style="background:..."`;
stack the text blocks vertically with hard-coded sizing; lower into
`PptxDeck`. **No CSS resolution.**

- Pro: zero new deps, single Rust file, instant.
- Pro: useful as a baseline for unit tests + closed-loop
  agent-driven exports of trivial HTML decks.
- **Con: not faithful** — Tailwind / CSS-grid / animations / web
  fonts / typography metrics all silently drop.

## Decision

- **v1 ships option (c)** as the `from_html` module — minimal but
  enough for the closed-loop demo, and provides a unit-test target
  that future browser-backed implementations can validate against.
- **v2+ adds option (b) — Tauri webview bridge** — when an explicit
  user-visible "Convert HTML to PPTX" workflow is built. Option (a)
  is rejected to avoid the 200 MB Chromium dep.

## Implementation pointers

### Current (option (c))

[`from_html.rs`](../../crates/design/design-export-pptx/src/from_html.rs)
- `<h1>/<h2>/<h3>/<p>` → `TextBox` with hard-coded sizes
- `<img src="data:image/...">` → `ImageBox`
- `<body style="background:..." data-slide-w="..." data-slide-h="...">`
  → slide background + dimensions
- Falls back to single placeholder shape on empty body

### Future (option (b))

When option (b) lands:

1. Add a Tauri command `pptx_render_html(html: String, opts) -> PptxLayout`
2. Open hidden webview tab; navigate to `data:text/html,<html>`
3. Inject extraction JS:
   ```js
   const layout = []
   document.querySelectorAll('*').forEach(el => {
     const r = el.getBoundingClientRect()
     const cs = getComputedStyle(el)
     layout.push({
       tag: el.tagName,
       rect: [r.x, r.y, r.width, r.height],
       text: el.innerText,
       style: { color: cs.color, font: cs.font, background: cs.background, /* ... */ },
     })
   })
   __TAURI__.event.emit('pptx-layout-ready', layout)
   ```
4. Rust receives the typed `Vec<HtmlElement>`, lowers into
   `PptxDeck` using the same `parse_css_color` + frame conversion
   utilities the existing writer relies on.

## Validation

- Phase 6d unit tests in `from_html::tests` cover the option (c)
  surface — once option (b) lands, those same fixtures should
  round-trip through it with at least equivalent fidelity (same
  text content, same bg color, same slide dimensions).

## Cross-references

- Plan: `kangnam-chat-luminous-teacup.md` § 6.4 (Phase 6d)
- Existing color path: ADR-006 (prompt cache) — unrelated; OKLch
  reduction lives in `color_convert.rs` (Phase 6c)
- huashu-design license: Personal-Use Only — we cannot vendor the
  `html2pptx.js` source, only port the algorithm

## Status update history

- 2026-05-01: Accepted with v1 = option (c), v2+ = option (b).
