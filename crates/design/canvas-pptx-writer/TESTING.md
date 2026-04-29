# Manual QA — canvas-pptx-writer

Before tagging any release, produce `/tmp/qa.pptx` from the `qa_deck()` helper in `tests/manual_qa.rs` (or hand-craft) and open it in:

- [ ] PowerPoint 365 (macOS) — text selectable; shapes movable; images display; gradient backgrounds render
- [ ] Apple Keynote 14+ — text stays in place; corners of rounded rectangles are correct
- [ ] Google Slides (upload, auto-convert) — all four shape types survive the conversion
- [ ] LibreOffice Impress — fallback check; no "file corrupted" dialog

If any viewer reports "needs repair", pull the repair log and diff against a known-good empty PowerPoint file.
