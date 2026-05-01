//! PPTX file-assembly layer.

use std::io::{Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::deck::{PptxDeck, PptxSlide};
use crate::element::{ImageBox, PptxElement};
use crate::error::{PptxWriteError, Result};

mod content_types;
mod core_props;
mod image;
mod layout;
mod master;
mod notes;
mod presentation;
mod relationships;
mod shape;
mod slide;
mod text;
mod theme;
mod xml;

pub fn write_deck(deck: &PptxDeck, out: &Path) -> Result<()> {
    let bytes = write_deck_to_bytes(deck)?;
    std::fs::write(out, bytes)?;
    Ok(())
}

pub fn write_deck_to_bytes(deck: &PptxDeck) -> Result<Vec<u8>> {
    if deck.slides.is_empty() { return Err(PptxWriteError::EmptyDeck); }
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip = ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let add = |zip: &mut ZipWriter<Cursor<&mut Vec<u8>>>, name: &str, bytes: &[u8]|
            -> Result<()> {
            zip.start_file(name, opts)?;
            zip.write_all(bytes)?;
            Ok(())
        };

        add(&mut zip, "[Content_Types].xml", &content_types::build(deck)?)?;
        add(&mut zip, "_rels/.rels", &relationships::package_rels()?)?;
        add(&mut zip, "docProps/core.xml", &core_props::core_xml(deck)?)?;
        add(&mut zip, "docProps/app.xml", &core_props::app_xml(deck)?)?;
        add(&mut zip, "ppt/presentation.xml", &presentation::build(deck)?)?;
        add(&mut zip, "ppt/_rels/presentation.xml.rels", &relationships::presentation_rels(deck)?)?;
        add(&mut zip, "ppt/theme/theme1.xml", &theme::build())?;
        add(&mut zip, "ppt/slideMasters/slideMaster1.xml", &master::build())?;
        add(&mut zip, "ppt/slideMasters/_rels/slideMaster1.xml.rels", &relationships::master_rels()?)?;
        add(&mut zip, "ppt/slideLayouts/slideLayout1.xml", &layout::build())?;
        add(&mut zip, "ppt/slideLayouts/_rels/slideLayout1.xml.rels", &relationships::layout_rels()?)?;

        // Global image counter — `imageN.ext` is deck-wide so each file is unique.
        let mut global_image_idx: u32 = 0;

        // Emit notesMaster if any slide has speaker_notes.
        let any_notes = deck.slides.iter().any(|s| s.speaker_notes.is_some());
        if any_notes {
            add(&mut zip, "ppt/notesMasters/notesMaster1.xml", &notes::notes_master_xml())?;
            add(
                &mut zip,
                "ppt/notesMasters/_rels/notesMaster1.xml.rels",
                &notes::notes_master_rels()?,
            )?;
        }

        for (idx, s) in deck.slides.iter().enumerate() {
            let n = idx + 1;
            let slide_images = collect_slide_images(idx, s, &mut global_image_idx)?;
            let has_notes = s.speaker_notes.is_some();

            // Slide XML
            add(&mut zip, &format!("ppt/slides/slide{n}.xml"), &slide::build(s, &slide_images)?)?;

            // Slide rels — image rels start at rId2; notesSlide rel
            // (when present) takes the next id after images.
            let rels_tuples: Vec<(String, String)> = slide_images.iter()
                .map(|si| (si.rel_id.clone(), si.filename.clone()))
                .collect();
            let notes_rel_id = if has_notes {
                Some(format!("rId{}", 2 + slide_images.len()))
            } else {
                None
            };
            add(
                &mut zip,
                &format!("ppt/slides/_rels/slide{n}.xml.rels"),
                &relationships::slide_rels_with_notes(&rels_tuples, notes_rel_id.as_deref(), n)?,
            )?;

            // Media bytes
            for si in &slide_images {
                add(&mut zip, &format!("ppt/media/{}", si.filename), &si.bytes)?;
            }

            // Notes slide
            if has_notes {
                add(
                    &mut zip,
                    &format!("ppt/notesSlides/notesSlide{n}.xml"),
                    &notes::notes_slide_xml(s),
                )?;
                add(
                    &mut zip,
                    &format!("ppt/notesSlides/_rels/notesSlide{n}.xml.rels"),
                    &notes::notes_slide_rels(n)?,
                )?;
            }
        }

        zip.finish()?;
    }
    Ok(buf)
}

/// Per-image bookkeeping used by slide + rels + media writers.
pub(crate) struct SlideImage {
    pub elem_idx: usize,        // position in slide.elements
    pub rel_id: String,         // e.g. "rId2"
    pub filename: String,       // e.g. "image1.png"
    pub bytes: Vec<u8>,
}

fn collect_slide_images(
    slide_idx: usize,
    s: &PptxSlide,
    global_counter: &mut u32,
) -> Result<Vec<SlideImage>> {
    let mut out: Vec<SlideImage> = Vec::new();
    for (eidx, el) in s.elements.iter().enumerate() {
        if let PptxElement::Image(img) = el {
            image::check_mime(&img.bytes, img.mime).map_err(|e| match e {
                PptxWriteError::MimeMismatch { .. } => e,
                other => PptxWriteError::InvalidImage {
                    slide_idx, element_idx: eidx, msg: other.to_string(),
                },
            })?;
            *global_counter += 1;
            // rId1 is slideLayout; image rels start at rId2 *per slide*.
            let per_slide_rid = 2 + out.len();
            let img_ref: &ImageBox = img;
            out.push(SlideImage {
                elem_idx: eidx,
                rel_id: format!("rId{per_slide_rid}"),
                filename: format!("image{}.{}", *global_counter, img_ref.mime.ext()),
                bytes: img_ref.bytes.clone(),
            });
        }
    }
    Ok(out)
}
