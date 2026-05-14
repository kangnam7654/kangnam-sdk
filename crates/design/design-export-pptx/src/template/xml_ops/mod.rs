//! XML manipulation helpers for the template path.
//!
//! These are byte-level string-splice operations on OOXML XML. They are
//! intentionally low-level — the goal is to mutate only the parts that need to
//! change, leaving the rest of the template (master, layouts, theme,
//! embedded fonts, etc.) byte-identical for round-trip preservation.
//!
//! # Module layout
//! - `util`          — `xml_err`, `splice_before`, `scan_max_rel_id`, `parse_attrs`, escape helpers
//! - `content_types` — `[Content_Types].xml` readers/mutators; `CT_SLIDE`, `CT_FONT`
//! - `relationships` — `.rels` readers/mutators; `REL_*` constants
//! - `presentation`  — `ppt/presentation.xml` readers/mutators
//! - `slide`         — slide XML builders, entry counters, `next_sp_id_in_slide`
//! - `placeholder`   — layout placeholder discovery, `PlaceholderMeta`, `XfrmRect`
//! - `shapes`        — picture/shape/text-box XML builders

mod content_types;
mod placeholder;
mod presentation;
mod relationships;
mod shapes;
mod slide;
mod util;

// ── Re-export all items used by template/mod.rs ───────────────────────────────

// Constants used by template/mod.rs
pub(super) use content_types::CT_FONT;
pub(super) use relationships::{REL_FONT, REL_IMAGE};

// content_types
pub(super) use content_types::{
    insert_content_types_default, insert_content_types_override,
    insert_content_types_override_for_slide, parse_declared_extensions,
};

// relationships
pub(super) use relationships::{
    append_presentation_rel, append_presentation_rel_for_slide, append_relationship,
    compute_next_rel_id,
};

// presentation
pub(super) use presentation::{parse_slide_size, update_sld_id_lst, upsert_embedded_font};

// slide
pub(super) use slide::{
    build_minimal_slide_xml, build_slide_rels, count_existing_fonts, count_existing_images,
    count_existing_slides, insert_before_sptree_close, next_sp_id_in_slide,
};

// placeholder — XfrmRect is constructed directly in template/mod.rs, so fields must be pub(crate)
pub(super) use placeholder::{
    XfrmRect, find_layout_placeholder, find_layout_placeholder_xfrm, upsert_slide_text_sp,
};

// shapes
pub(super) use shapes::{
    build_free_text_sp_xml, build_geom_shape_xml, build_pic_xml, build_shape_sp_xml,
    image_mime_for_ext, prst_geom_xml, stroke_xml,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_16_9_slide_size() {
        let xml = br#"<?xml version="1.0"?><p:presentation xmlns:p="x">
            <p:sldSz cx="12192000" cy="6858000"/>
        </p:presentation>"#;
        assert_eq!(parse_slide_size(xml), Some((12_192_000, 6_858_000)));
    }

    #[test]
    fn returns_none_when_sldsz_missing() {
        let xml = br#"<p:presentation/>"#;
        assert_eq!(parse_slide_size(xml), None);
    }

    #[test]
    fn build_minimal_tx_body_splits_lines() {
        let body = placeholder::build_minimal_tx_body("a\nb\nc");
        assert_eq!(body.matches("<a:p>").count(), 3);
        assert!(body.contains(">a<"));
        assert!(body.contains(">b<"));
    }

    #[test]
    fn escape_xml_text_handles_specials() {
        assert_eq!(
            util::escape_xml_text("a & b < c > d"),
            "a &amp; b &lt; c &gt; d"
        );
    }

    #[test]
    fn parse_attrs_handles_multiple() {
        let attrs = util::parse_attrs(r#"<p:ph type="ctrTitle" idx="0"/>"#);
        assert!(attrs.iter().any(|(k, v)| k == "type" && v == "ctrTitle"));
        assert!(attrs.iter().any(|(k, v)| k == "idx" && v == "0"));
    }

    #[test]
    fn next_sp_id_finds_max_plus_one() {
        let xml = br#"<p:cNvPr id="1"/><p:cNvPr id="42"/><p:cNvPr id="7"/>"#;
        assert_eq!(next_sp_id_in_slide(xml), 43);
    }

    #[test]
    fn append_relationship_returns_next_rid() {
        let rels = br#"<?xml version="1.0"?><Relationships xmlns="x">
            <Relationship Id="rId1" Type="t" Target="a"/>
            <Relationship Id="rId3" Type="t" Target="b"/>
            </Relationships>"#;
        let (out, id) = append_relationship(rels, "type-z", "target-z").unwrap();
        assert_eq!(id, "rId4");
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains(r#"Id="rId4""#));
        assert!(s.contains(r#"Type="type-z""#));
    }
}
