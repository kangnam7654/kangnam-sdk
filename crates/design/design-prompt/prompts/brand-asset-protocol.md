# Brand asset protocol — 5 step (mandatory when a brand is named)

> The single biggest reduction in AI-slop variance: **never paint a brand
> from memory; always extract from primary sources first.**

This block fires when the user names a specific brand or product (Stripe,
Linear, Anthropic, DJI, Nano Banana Pro, the user's own company …). It runs
before any layout.

## Why this is hard-coded

Models routinely "remember" brand colors as bland averages
(`#1E40AF`-ish), forget logos exist, and substitute hand-drawn SVG humans
for product photography. The protocol forces a real-world detour through
five concrete steps so the output reads like the brand actually intended.

## Step 0 — fact verification (precedes the protocol)

Before extracting brand assets, confirm the brand / product is what you
think it is:

- Use `WebSearch` for any specific product / version / spec / launch date
  the user mentions and you don't already know with certainty.
- Read 1–3 authoritative results (official site, recent press, GitHub repo).
- If you find a release date, version number, or feature you didn't expect,
  **trust the search result, not your memory.**

If you can't verify the brand exists, **ask the user** before continuing.
Producing a "concept demo" for a real product is worse than asking.

## Step 1 — locate primary sources

In order:

1. Files the user attached (CSS, brand guide PDFs, screenshots).
2. Their website's brand / press page: `<brand>.com/brand`,
   `<brand>.com/press`, `<brand>.com/about`, `<brand>.com/style-guide`.
3. Their public design system if any: `<brand>.com/design`,
   `design.<brand>.com`.
4. App-store listings if it's a mobile-app brand.
5. Their official social media (Twitter/X, Instagram) for the latest logo
   crop.

Use `WebFetch` (or in-tool fetch) — not memory.

## Step 2 — download styling artefacts

- CSS / Tailwind config / brand-guide PDF.
- Logo at multiple weights (mark + wordmark).
- Typography references (Google Fonts URLs, Adobe Fonts CSS).
- Hero photography / product shots.

Save under `assets/brand/` in the project. Reference by relative path; never
inline a 50KB SVG into a `<style>` block.

## Step 3 — extract real values

Run `grep -E '#[0-9a-fA-F]{3,8}'` on every CSS file. Note frequency. The
top 6 distinct hex / oklch values (excluding pure black + pure white +
common greys) are usually the brand's palette.

For typography, eyeball screenshots: identify display vs body weights,
note custom fonts (Inter, GT America, Söhne …), capture the font-feature
settings (tabular-nums, contextual swashes).

For layout posture, observe: border-radius (sharp / soft), border weight
(hairline / 1px / 2px+), accent budget (one bold color or many?).

## Step 4 — codify into `brand-spec.md`

Write a single file at the project root:

```md
# Brand spec — <Brand>

## Tokens (OKLch — convert from hex)
- `--bg`:      oklch(...)
- `--surface`: oklch(...)
- `--fg`:      oklch(...)
- `--muted`:   oklch(...)
- `--border`:  oklch(...)
- `--accent`:  oklch(...)

## Typography
- Display: <font stack>
- Body: <font stack>
- Mono: <font stack>

## Layout posture
- Radius: <e.g. 8–12px on cards, 0 on buttons>
- Border: <hairline / 1.5px solid fg / etc.>
- Accent budget: <"one bold color, used at most twice per screen">
- <other observations>

## Sources
- <brand>.com/brand — fetched 2026-04-30
- screenshots/<file>.png
- <other>
```

Every layout that follows references **this file**, not the model's memory.

## Step 5 — vocalise the system

Before writing CSS, state the system in one sentence so the user can
redirect cheaply:

> "Warm cream background (oklch 97% 0.018 70), single rust accent
> (oklch 64% 0.13 28), Newsreader display + system body, gentle 12px
> radii. Sound right?"

If the user disagrees, fix `brand-spec.md` first; do not start layout.

---

> Distilled from open-design's brand-extraction directive in
> [discovery.ts](https://github.com/nexu-io/open-design/blob/main/src/prompts/discovery.ts)
> (Apache-2.0). Five-step structure inspired by alchaincyf/huashu-design
> (Personal-Use License — ideas only, no verbatim text).
