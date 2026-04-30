//! Template-editing path: load an existing `.pptx`, append slides that inherit
//! from layouts, fill placeholders, and re-zip — preserving untouched entries
//! byte-identical.
//!
//! This is the counterpart to the write-only [`crate::PptxDeck`] IR. Use this
//! when your presentation depends on a designed template (slideMaster +
//! slideLayouts + theme + embedded fonts authored in PowerPoint/Keynote) and
//! code only needs to inject variable bits.
//!
//! # Example
//!
//! ```no_run
//! use design_export_pptx::{PptxTemplate, ImageMime, FontVariant};
//! use std::path::Path;
//!
//! let mut t = PptxTemplate::load(Path::new("templates/resume.pptx"))?;
//! let s = t.add_slide_from_layout(1)?;
//! t.set_placeholder_text(s, 0, "홍길동")?;
//! t.set_placeholder_text(s, 1, "Senior Engineer")?;
//! // Embed a custom typeface so the viewer doesn't need it installed:
//! // let pretendard = std::fs::read("Pretendard-Regular.ttf")?;
//! // t.embed_font("Pretendard", FontVariant::Regular, pretendard)?;
//! let bytes = t.pack()?;
//! # Ok::<(), design_export_pptx::PptxWriteError>(())
//! ```

mod xml_ops;

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::color::blip_tile_fill_xml;
use crate::element::{ImageMime, PptxElement};
use crate::error::{PptxWriteError, Result};
use crate::geometry::Frame;

use xml_ops::{
    append_presentation_rel, append_presentation_rel_for_slide, append_relationship,
    build_free_text_sp_xml, build_geom_shape_xml, build_minimal_slide_xml, build_pic_xml,
    build_shape_sp_xml, build_slide_rels, compute_next_rel_id, count_existing_fonts,
    count_existing_images, count_existing_slides, find_layout_placeholder,
    find_layout_placeholder_xfrm, image_mime_for_ext, insert_before_sptree_close,
    insert_content_types_default, insert_content_types_override,
    insert_content_types_override_for_slide, next_sp_id_in_slide,
    parse_declared_extensions, parse_slide_size, prst_geom_xml, stroke_xml as shape_stroke_xml,
    update_sld_id_lst, upsert_embedded_font, upsert_slide_text_sp, XfrmRect, CT_FONT, REL_FONT,
    REL_IMAGE,
};

/// Font weight/style variant for [`PptxTemplate::embed_font`].
///
/// Maps to the OOXML element names used inside `<p:embeddedFont>`:
/// `regular`, `bold`, `italic`, `boldItalic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontVariant {
    /// Regular (upright, normal weight). OOXML tag: `<p:regular>`.
    Regular,
    /// Bold weight. OOXML tag: `<p:bold>`.
    Bold,
    /// Italic style. OOXML tag: `<p:italic>`.
    Italic,
    /// Bold and italic combined. OOXML tag: `<p:boldItalic>`.
    BoldItalic,
}

impl FontVariant {
    /// Returns the exact OOXML element name for this variant.
    pub(super) fn ooxml_tag(self) -> &'static str {
        match self {
            FontVariant::Regular => "regular",
            FontVariant::Bold => "bold",
            FontVariant::Italic => "italic",
            FontVariant::BoldItalic => "boldItalic",
        }
    }
}

/// 1-based slide number returned by [`PptxTemplate::add_slide_from_layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlideRef(pub usize);

/// In-memory editable PPTX. Holds raw zip entries and mutates them in place.
///
/// Construct via [`PptxTemplate::load`] or [`PptxTemplate::load_bytes`].
#[derive(Debug)]
pub struct PptxTemplate {
    /// Zip entries in original order. Untouched entries are byte-identical
    /// after [`PptxTemplate::pack`].
    entries: Vec<(String, Vec<u8>)>,
    /// Total slide count in the package (existing + appended). Used as the
    /// next slide number when appending.
    slide_count: usize,
    /// Layout num for slides we appended, keyed by absolute slide_num.
    /// Pre-existing slides (loaded with the template) are not in this map —
    /// callers cannot construct a `SlideRef` pointing at them.
    slide_layout_nums: HashMap<usize, usize>,
    /// Next available `rId` in `ppt/_rels/presentation.xml.rels`.
    next_pres_rel_id: usize,
    /// Counter for `ppt/media/imageN.ext`.
    image_count: usize,
    /// File extensions already declared in `[Content_Types].xml`.
    declared_extensions: HashSet<String>,
    /// Number of fonts embedded via [`PptxTemplate::embed_font`].
    /// Pre-existing `ppt/fonts/fontN.fntdata` entries are counted at load time
    /// so newly embedded fonts get unique IDs.
    font_count: usize,
}

impl PptxTemplate {
    /// Load a `.pptx` from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let archive = zip::ZipArchive::new(file)?;
        Self::from_archive(archive)
    }

    /// Load a `.pptx` from an in-memory byte buffer.
    pub fn load_bytes(bytes: &[u8]) -> Result<Self> {
        let cursor = Cursor::new(bytes);
        let archive = zip::ZipArchive::new(cursor)?;
        Self::from_archive(archive)
    }

    fn from_archive<R: Read + std::io::Seek>(mut archive: zip::ZipArchive<R>) -> Result<Self> {
        let mut entries = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            entries.push((name, buf));
        }

        if !entries.iter().any(|(n, _)| n == "ppt/presentation.xml") {
            return Err(PptxWriteError::InvalidTemplate {
                reason: "missing ppt/presentation.xml".into(),
                detail: "not a valid OOXML presentation".into(),
            });
        }
        if !entries.iter().any(|(n, _)| n == "[Content_Types].xml") {
            return Err(PptxWriteError::InvalidTemplate {
                reason: "missing [Content_Types].xml".into(),
                detail: "not a valid OOXML package".into(),
            });
        }

        let next_pres_rel_id = compute_next_rel_id(&entries, "ppt/_rels/presentation.xml.rels")?;
        let declared_extensions = parse_declared_extensions(&entries)?;
        let image_count = count_existing_images(&entries);
        // Pre-existing slides occupy slide1..slideN. Newly appended slides
        // start at N+1, so seed slide_count from the existing count.
        let slide_count = count_existing_slides(&entries);
        // Pre-existing font entries determine the next font index.
        let font_count = count_existing_fonts(&entries);

        Ok(Self {
            entries,
            slide_count,
            slide_layout_nums: HashMap::new(),
            next_pres_rel_id,
            image_count,
            declared_extensions,
            font_count,
        })
    }

    /// Number of `slideLayoutN.xml` entries discovered in the template
    /// (1-based max layout index).
    pub fn layout_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|(name, _)| {
                name.starts_with("ppt/slideLayouts/slideLayout")
                    && name.ends_with(".xml")
                    && !name.contains("_rels")
            })
            .count()
    }

    /// Slide dimensions from `<p:sldSz cx=".." cy=".."/>` in
    /// `ppt/presentation.xml`. Falls back to (12_192_000, 6_858_000) (16:9 EMU)
    /// if not parseable.
    pub fn slide_size_emu(&self) -> (i64, i64) {
        let pres = self
            .entry_bytes("ppt/presentation.xml")
            .unwrap_or(&[]);
        parse_slide_size(pres).unwrap_or((12_192_000, 6_858_000))
    }

    /// Append a new slide that inherits from `slideLayoutN.xml`.
    pub fn add_slide_from_layout(&mut self, layout_num: usize) -> Result<SlideRef> {
        let max = self.layout_count();
        if layout_num == 0 || layout_num > max {
            return Err(PptxWriteError::LayoutNotFound { layout_num, max });
        }

        let slide_xml = build_minimal_slide_xml();
        self.slide_count += 1;
        let slide_num = self.slide_count;
        self.slide_layout_nums.insert(slide_num, layout_num);
        let rel_id = self.next_pres_rel_id;
        self.next_pres_rel_id += 1;

        let slide_path = format!("ppt/slides/slide{}.xml", slide_num);
        let slide_rels_path = format!("ppt/slides/_rels/slide{}.xml.rels", slide_num);

        self.entries.push((slide_path, slide_xml.into_bytes()));
        self.entries
            .push((slide_rels_path, build_slide_rels(layout_num).into_bytes()));

        // Mutate [Content_Types].xml
        let ct = self
            .entry_bytes_required("[Content_Types].xml")?
            .to_vec();
        let new_ct = insert_content_types_override_for_slide(&ct, slide_num)?;
        self.set_entry("[Content_Types].xml", new_ct);

        // Mutate ppt/_rels/presentation.xml.rels
        let rels_path = "ppt/_rels/presentation.xml.rels";
        let rels = self.entry_bytes_required(rels_path)?.to_vec();
        let new_rels = append_presentation_rel_for_slide(&rels, rel_id, slide_num)?;
        self.set_entry(rels_path, new_rels);

        // Mutate ppt/presentation.xml
        let pres_path = "ppt/presentation.xml";
        let pres = self.entry_bytes_required(pres_path)?.to_vec();
        let new_pres = update_sld_id_lst(&pres, self.slide_count, rel_id)?;
        self.set_entry(pres_path, new_pres);

        Ok(SlideRef(slide_num))
    }

    /// Inject plain text into the layout placeholder by `idx`. Newlines in
    /// `text` produce multiple `<a:p>` paragraphs. Formatting is inherited
    /// from the layout.
    pub fn set_placeholder_text(
        &mut self,
        slide: SlideRef,
        placeholder_idx: u32,
        text: &str,
    ) -> Result<()> {
        let layout_xml = self.layout_xml_for_slide(slide)?.to_vec();
        let layout_num = self.layout_num_for_slide(slide)?;
        let ph_meta = find_layout_placeholder(&layout_xml, placeholder_idx)?
            .ok_or(PptxWriteError::PlaceholderNotFound {
                layout_num,
                placeholder_idx,
            })?;

        let slide_path = self.slide_path(slide);
        let slide_xml = self.entry_bytes_required(&slide_path)?.to_vec();
        let new_xml = upsert_slide_text_sp(&slide_xml, &ph_meta, text)?;
        self.set_entry(&slide_path, new_xml);
        Ok(())
    }

    /// Fill a layout placeholder with an image, using the placeholder's
    /// geometry from the layout (position/size inherited).
    pub fn set_placeholder_image(
        &mut self,
        slide: SlideRef,
        placeholder_idx: u32,
        image_bytes: &[u8],
        mime: ImageMime,
    ) -> Result<()> {
        let layout_xml = self.layout_xml_for_slide(slide)?.to_vec();
        let xfrm = find_layout_placeholder_xfrm(&layout_xml, placeholder_idx)?
            .unwrap_or(XfrmRect {
                off_x: 0,
                off_y: 0,
                ext_cx: 5_486_400,
                ext_cy: 3_657_600,
            });
        self.add_picture_at(slide, image_bytes, mime, xfrm)
    }

    /// Add a freeform element at absolute EMU coordinates. Placeholder-independent.
    ///
    /// Supports `PptxElement::Text` (rich text attrs: italic, color_alpha,
    /// allow_wrap, line_height, letter_spacing), `PptxElement::Shape` (all
    /// `Fill` variants — solid, linear/radial gradient, tile pattern, none —
    /// plus `Stroke` and `OuterShadow`), and `PptxElement::Image`.
    pub fn add_element(&mut self, slide: SlideRef, element: PptxElement) -> Result<()> {
        match element {
            PptxElement::Image(img) => {
                let mime = img.mime;
                let xfrm = XfrmRect {
                    off_x: img.frame.x_emu,
                    off_y: img.frame.y_emu,
                    ext_cx: img.frame.w_emu,
                    ext_cy: img.frame.h_emu,
                };
                self.add_picture_at(slide, &img.bytes, mime, xfrm)
            }
            PptxElement::Text(tb) => {
                // Validate slide exists.
                let _ = self.layout_num_for_slide(slide)?;

                let slide_path = self.slide_path(slide);
                let slide_xml = self.entry_bytes_required(&slide_path)?.to_vec();
                let sp_id = next_sp_id_in_slide(&slide_xml);
                let sp_xml = build_free_text_sp_xml(sp_id, &tb);
                let new_slide_xml = insert_before_sptree_close(&slide_xml, &sp_xml)?;
                self.set_entry(&slide_path, new_slide_xml);
                Ok(())
            }
            PptxElement::Shape(sb) => {
                // Validate slide exists.
                let _ = self.layout_num_for_slide(slide)?;

                let xfrm = XfrmRect {
                    off_x: sb.frame.x_emu,
                    off_y: sb.frame.y_emu,
                    ext_cx: sb.frame.w_emu,
                    ext_cy: sb.frame.h_emu,
                };

                // Build fill XML. TilePattern requires PNG embed + slide rels mutation.
                let fill_xml = match &sb.fill {
                    crate::color::Fill::TilePattern { png_bytes, .. } => {
                        // Embed PNG.
                        self.image_count += 1;
                        let image_num = self.image_count;
                        let image_path = format!("ppt/media/image{}.png", image_num);
                        self.entries.push((image_path, png_bytes.clone()));

                        // Register <Default Extension="png"> if not yet declared.
                        if !self.declared_extensions.contains("png") {
                            let ct = self.entry_bytes_required("[Content_Types].xml")?.to_vec();
                            let new_ct = insert_content_types_default(
                                &ct,
                                "png",
                                image_mime_for_ext("png"),
                            )?;
                            self.set_entry("[Content_Types].xml", new_ct);
                            self.declared_extensions.insert("png".to_string());
                        }

                        // Append image relationship to slide rels.
                        let slide_rels_path = self.slide_rels_path(slide);
                        let rels_xml = self.entry_bytes_required(&slide_rels_path)?.to_vec();
                        let (new_rels, image_rel_id) = append_relationship(
                            &rels_xml,
                            REL_IMAGE,
                            &format!("../media/image{}.png", image_num),
                        )?;
                        self.set_entry(&slide_rels_path, new_rels);

                        blip_tile_fill_xml(&image_rel_id)
                    }
                    fill => fill.to_ooxml_fill_xml(),
                };

                // For Line shapes, force noFill (lines have no interior).
                let fill_xml = match sb.shape {
                    crate::element::ShapeKind::Line => "<a:noFill/>".to_string(),
                    _ => fill_xml,
                };

                let geom_xml = prst_geom_xml(&sb.shape, sb.frame.w_emu, sb.frame.h_emu);
                let stroke_xml_str = shape_stroke_xml(&sb.stroke);
                let effect_xml = match &sb.shadow {
                    Some(s) => s.to_ooxml_effect_xml(),
                    None => String::new(),
                };

                let slide_path = self.slide_path(slide);
                let slide_xml = self.entry_bytes_required(&slide_path)?.to_vec();
                let sp_id = next_sp_id_in_slide(&slide_xml);
                let sp_xml = build_shape_sp_xml(
                    sp_id,
                    &xfrm,
                    &geom_xml,
                    &fill_xml,
                    &stroke_xml_str,
                    &effect_xml,
                );
                let new_slide_xml = insert_before_sptree_close(&slide_xml, &sp_xml)?;
                self.set_entry(&slide_path, new_slide_xml);
                Ok(())
            }
        }
    }

    /// Add a full-slide image using the template's declared slide size.
    pub fn add_full_bleed_image(
        &mut self,
        slide: SlideRef,
        image_bytes: &[u8],
        mime: ImageMime,
    ) -> Result<()> {
        let (w, h) = self.slide_size_emu();
        let xfrm = XfrmRect {
            off_x: 0,
            off_y: 0,
            ext_cx: w,
            ext_cy: h,
        };
        self.add_picture_at(slide, image_bytes, mime, xfrm)
    }

    /// Embed a custom font into the PPTX so that PowerPoint/Keynote/Google Slides
    /// can render the typeface even when the viewer's system has no copy installed.
    ///
    /// Calling this multiple times with the same `typeface` and different
    /// `variant`s merges them into one `<p:embeddedFont>` block, matching how
    /// PowerPoint authors a typeface family.
    ///
    /// # Arguments
    /// - `typeface`: PPT face name (must exactly match the `<a:latin typeface="..."/>`
    ///   used in your slide text — `"Pretendard"` for Pretendard, etc.).
    /// - `variant`: which weight/style this `ttf_bytes` represents.
    /// - `ttf_bytes`: raw TTF or OTF (woff2 **not** supported — strip the wrapper first).
    ///
    /// # What it mutates (all in-memory; written to zip on [`pack`](Self::pack))
    /// 1. Appends `ppt/fonts/fontN.fntdata` with the raw `ttf_bytes`.
    /// 2. Adds a `<Override>` entry for that part in `[Content_Types].xml`.
    /// 3. Adds a `<Relationship Type=".../font">` in `ppt/_rels/presentation.xml.rels`.
    /// 4. Upserts `<p:embeddedFontLst>` in `ppt/presentation.xml`.
    ///
    /// # Errors
    /// - [`PptxWriteError::Xml`] if `presentation.xml` is malformed.
    /// - [`PptxWriteError::Io`] on I/O failure (not expected for in-memory archives).
    ///
    /// # Example
    /// ```no_run
    /// use design_export_pptx::{PptxTemplate, FontVariant};
    /// # fn main() -> design_export_pptx::Result<()> {
    /// let mut tmpl = PptxTemplate::load_bytes(&[])?;
    /// let pretendard = std::fs::read("Pretendard-Regular.ttf").unwrap();
    /// tmpl.embed_font("Pretendard", FontVariant::Regular, pretendard)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn embed_font(
        &mut self,
        typeface: &str,
        variant: FontVariant,
        ttf_bytes: Vec<u8>,
    ) -> Result<()> {
        self.font_count += 1;
        let idx = self.font_count;
        let font_path = format!("ppt/fonts/font{}.fntdata", idx);

        // 1. Add the raw font bytes as a new zip entry.
        self.entries.push((font_path.clone(), ttf_bytes));

        // 2. Add <Override> to [Content_Types].xml.
        let part_name = format!("/{}", font_path);
        let ct = self.entry_bytes_required("[Content_Types].xml")?.to_vec();
        let new_ct = insert_content_types_override(&ct, &part_name, CT_FONT)?;
        self.set_entry("[Content_Types].xml", new_ct);

        // 3. Add <Relationship> to ppt/_rels/presentation.xml.rels.
        let rel_id = self.next_pres_rel_id;
        self.next_pres_rel_id += 1;
        let rels_path = "ppt/_rels/presentation.xml.rels";
        let rels = self.entry_bytes_required(rels_path)?.to_vec();
        let font_target = format!("fonts/font{}.fntdata", idx);
        let new_rels = append_presentation_rel(&rels, rel_id, REL_FONT, &font_target)?;
        self.set_entry(rels_path, new_rels);

        // 4. Upsert <p:embeddedFontLst> in ppt/presentation.xml.
        let pres_path = "ppt/presentation.xml";
        let pres = self.entry_bytes_required(pres_path)?.to_vec();
        let new_pres = upsert_embedded_font(&pres, typeface, variant.ooxml_tag(), rel_id)?;
        self.set_entry(pres_path, new_pres);

        Ok(())
    }

    /// Zip and return the final `.pptx` bytes. Consumes self.
    pub fn pack(self) -> Result<Vec<u8>> {
        let mut buf = Cursor::new(Vec::<u8>::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let options: zip::write::SimpleFileOptions =
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
            for (name, bytes) in &self.entries {
                zw.start_file(name, options)?;
                zw.write_all(bytes)?;
            }
            zw.finish()?;
        }
        Ok(buf.into_inner())
    }

    /// Convenience: pack and write to `path`.
    pub fn write(self, path: &Path) -> Result<()> {
        let bytes = self.pack()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Add a tile-pattern filled rectangle at absolute EMU coordinates.
    ///
    /// # Deprecated since v0.3.4
    /// Use [`add_element`](Self::add_element) with
    /// `PptxElement::Shape(ShapeBox { fill: Fill::TilePattern { .. }, .. })`
    /// instead. The implementation is preserved; it will be removed in v0.4.0.
    ///
    /// # Arguments
    /// - `slide` — slide returned by [`add_slide_from_layout`](Self::add_slide_from_layout).
    /// - `frame` — EMU position and size of the shape.
    /// - `png_bytes` — raw PNG bytes to embed as the tile.
    /// - `border_radius_emu` — corner radius in EMU; 0 for a plain rectangle.
    ///
    /// # Errors
    /// - [`PptxWriteError::SlideNotFound`] if `slide` was not created by this template.
    /// - [`PptxWriteError::Xml`] if required zip entries are missing or malformed.
    #[deprecated(
        since = "0.3.4",
        note = "use add_element(PptxElement::Shape(ShapeBox { fill: Fill::TilePattern{..}, .. }))"
    )]
    pub fn add_tile_pattern_rect(
        &mut self,
        slide: SlideRef,
        frame: Frame,
        png_bytes: &[u8],
        border_radius_emu: i64,
    ) -> Result<()> {
        // Validate slide.
        let _ = self.layout_num_for_slide(slide)?;

        // Embed PNG into ppt/media/imageN.png.
        self.image_count += 1;
        let image_num = self.image_count;
        let image_path = format!("ppt/media/image{}.png", image_num);
        self.entries.push((image_path, png_bytes.to_vec()));

        // Register <Default Extension="png"> if not already declared.
        if !self.declared_extensions.contains("png") {
            let ct = self.entry_bytes_required("[Content_Types].xml")?.to_vec();
            let new_ct =
                insert_content_types_default(&ct, "png", image_mime_for_ext("png"))?;
            self.set_entry("[Content_Types].xml", new_ct);
            self.declared_extensions.insert("png".to_string());
        }

        // Append image relationship to slide rels.
        let slide_rels_path = self.slide_rels_path(slide);
        let rels_xml = self.entry_bytes_required(&slide_rels_path)?.to_vec();
        let (new_rels, image_rel_id) = append_relationship(
            &rels_xml,
            REL_IMAGE,
            &format!("../media/image{}.png", image_num),
        )?;
        self.set_entry(&slide_rels_path, new_rels);

        // Emit <p:sp> with <a:blipFill><a:tile/>.
        let slide_path = self.slide_path(slide);
        let slide_xml = self.entry_bytes_required(&slide_path)?.to_vec();
        let sp_id = next_sp_id_in_slide(&slide_xml);
        let xfrm = XfrmRect {
            off_x: frame.x_emu,
            off_y: frame.y_emu,
            ext_cx: frame.w_emu,
            ext_cy: frame.h_emu,
        };
        let fill_xml = blip_tile_fill_xml(&image_rel_id);
        let sp_xml = build_geom_shape_xml(
            sp_id,
            &xfrm,
            border_radius_emu,
            frame.w_emu,
            frame.h_emu,
            &fill_xml,
        );
        let new_slide_xml = insert_before_sptree_close(&slide_xml, &sp_xml)?;
        self.set_entry(&slide_path, new_slide_xml);
        Ok(())
    }

    // ── internal helpers ───────────────────────────────────────────────

    fn entry_index(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|(n, _)| n == name)
    }

    fn entry_bytes(&self, name: &str) -> Option<&[u8]> {
        self.entry_index(name).map(|i| self.entries[i].1.as_slice())
    }

    fn entry_bytes_required(&self, name: &str) -> Result<&[u8]> {
        self.entry_bytes(name).ok_or_else(|| PptxWriteError::Xml(format!("entry missing: {name}")))
    }

    fn set_entry(&mut self, name: &str, bytes: Vec<u8>) {
        if let Some(i) = self.entry_index(name) {
            self.entries[i].1 = bytes;
        } else {
            self.entries.push((name.to_string(), bytes));
        }
    }

    fn slide_path(&self, slide: SlideRef) -> String {
        format!("ppt/slides/slide{}.xml", slide.0)
    }

    fn slide_rels_path(&self, slide: SlideRef) -> String {
        format!("ppt/slides/_rels/slide{}.xml.rels", slide.0)
    }

    fn layout_num_for_slide(&self, slide: SlideRef) -> Result<usize> {
        self.slide_layout_nums
            .get(&slide.0)
            .copied()
            .ok_or(PptxWriteError::SlideNotFound { slide_num: slide.0 })
    }

    fn layout_xml_for_slide(&self, slide: SlideRef) -> Result<&[u8]> {
        let layout_num = self.layout_num_for_slide(slide)?;
        let name = format!("ppt/slideLayouts/slideLayout{}.xml", layout_num);
        self.entry_bytes(&name)
            .ok_or_else(|| PptxWriteError::Xml(format!("layout XML missing: {name}")))
    }

    fn add_picture_at(
        &mut self,
        slide: SlideRef,
        image_bytes: &[u8],
        mime: ImageMime,
        xfrm: XfrmRect,
    ) -> Result<()> {
        // Validate slide exists.
        let _ = self.layout_num_for_slide(slide)?;

        let ext = mime.ext().to_string();
        self.image_count += 1;
        let image_num = self.image_count;
        let image_path = format!("ppt/media/image{}.{}", image_num, ext);
        self.entries.push((image_path, image_bytes.to_vec()));

        if !self.declared_extensions.contains(&ext) {
            let ct_bytes = self.entry_bytes_required("[Content_Types].xml")?.to_vec();
            let new_ct = insert_content_types_default(&ct_bytes, &ext, image_mime_for_ext(&ext))?;
            self.set_entry("[Content_Types].xml", new_ct);
            self.declared_extensions.insert(ext.clone());
        }

        let slide_rels_path = self.slide_rels_path(slide);
        let rels_xml = self.entry_bytes_required(&slide_rels_path)?.to_vec();
        let (new_rels, image_rel_id) = append_relationship(
            &rels_xml,
            REL_IMAGE,
            &format!("../media/image{}.{}", image_num, ext),
        )?;
        self.set_entry(&slide_rels_path, new_rels);

        let slide_path = self.slide_path(slide);
        let slide_xml = self.entry_bytes_required(&slide_path)?.to_vec();
        let next_sp_id = xml_ops::next_sp_id_in_slide(&slide_xml);
        let pic_xml = build_pic_xml(&image_rel_id, next_sp_id, &xfrm);
        let new_slide_xml = insert_before_sptree_close(&slide_xml, &pic_xml)?;
        self.set_entry(&slide_path, new_slide_xml);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_bytes_rejects_non_zip() {
        let err = PptxTemplate::load_bytes(b"not a zip").unwrap_err();
        assert!(
            matches!(err, PptxWriteError::Zip(_)),
            "expected Zip error, got {err:?}"
        );
    }

    #[test]
    fn load_bytes_rejects_zip_without_presentation_xml() {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(Cursor::new(&mut buf));
            zw.start_file("foo.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"hello").unwrap();
            zw.finish().unwrap();
        }
        let err = PptxTemplate::load_bytes(&buf).unwrap_err();
        assert!(
            matches!(err, PptxWriteError::InvalidTemplate { .. }),
            "expected InvalidTemplate, got {err:?}"
        );
    }
}
