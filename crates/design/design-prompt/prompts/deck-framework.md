# Deck framework directive (load-bearing — pinned LAST)

When the active artifact is a deck (`metadata.kind == "deck"` OR the active
skill is a deck-mode skill), wrap the slides in this framework so the
host can navigate, the user can scroll-snap, and PDF export stitches
correctly. **This block overrides any softer slide-handling wording earlier
in the prompt.**

The active skill may ship its own seed template (e.g. guizang-ppt's
`assets/template.html`); when present, use the skill's seed and skip the
generic skeleton below — the skill's framework wins.

## Required scaffolding (when no skill seed)

```html
<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=1920">
  <title>{{ deck_title }}</title>
  <style>
    :root { /* design system tokens go here */ }
    * { box-sizing: border-box; }
    html, body { margin: 0; padding: 0; background: var(--bg); font-family: var(--body-font); }
    body { scroll-snap-type: y mandatory; overflow-y: auto; height: 100vh; }
    section.slide {
      position: relative;
      width: 1920px;
      height: 1080px;
      margin: 0 auto;
      overflow: hidden;
      scroll-snap-align: start;
    }
    @media print {
      body { scroll-snap-type: none; height: auto; }
      section.slide { page-break-after: always; }
    }
  </style>
</head>
<body>
  <nav class="deck-nav" aria-label="slides">
    <!-- prev / next / counter (slide N of M) -->
  </nav>
  <section class="slide" data-slide-index="0" data-slide-id="cover">
    <!-- slide content -->
  </section>
  <!-- … more slides -->
  <script>
    // arrow-key + space navigation, counter sync, section.scrollIntoView
  </script>
</body>
</html>
```

## Hard contracts the framework MUST satisfy

1. **Section-per-slide** — each slide is a `<section class="slide">` so
   `scrollIntoView({block: 'start'})` snaps cleanly and PDF export gets one
   slide per page.
2. **Fixed canvas** — width × height in CSS pixels. 1920×1080 default for
   modern decks, 1280×720 for legacy. Don't do responsive slides.
3. **Scroll-snap** — `body { scroll-snap-type: y mandatory }` plus
   `section.slide { scroll-snap-align: start }`. Without these, the
   navigator jumps mid-slide.
4. **Arrow-key navigation** — JS listener for `ArrowDown`/`ArrowUp`/
   `Space`/`PageDown`/`PageUp`. Map to next / prev section.
5. **Slide counter** — `<span data-current>1</span> / <span data-total>N</span>`
   updated on scroll. The user always knows where they are.
6. **`@media print`** — disable scroll-snap, force `page-break-after: always`
   on each slide. PDF export is a deck export.
7. **`data-slide-index` and `data-slide-id`** — the host's preview overlay
   uses these to highlight or focus a specific slide.

## What NOT to do in deck mode

- Don't use `place-items: center` with a transform — it conflicts with
  scroll-snap. Use absolute positioning inside the section instead.
- Don't use `vh`/`vw` for slide-internal sizing. Slides have a fixed canvas;
  use pixel values so PDF export matches preview.
- Don't add a sticky nav inside the slide — the deck nav lives at body level.
- Don't make slides scrollable — overflow is hidden, content fits.

## Speaker notes

If `metadata.speakerNotes` is `true`, every `<section class="slide">` should
have a `<aside class="speaker-notes" data-slide-id="...">` sibling outside
the visible canvas (e.g. `position: absolute; left: -9999px;`). The PPTX
exporter and presenter mode both consume these.

> Distilled from open-design's `DECK_FRAMEWORK_DIRECTIVE`
> ([src/prompts/deck-framework.ts](https://github.com/nexu-io/open-design/blob/main/src/prompts/deck-framework.ts),
> Apache-2.0).
