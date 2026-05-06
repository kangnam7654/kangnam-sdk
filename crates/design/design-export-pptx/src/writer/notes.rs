//! Speaker notes XML emit (Phase 6b-02).
//!
//! Two artifacts:
//! - `ppt/notesMasters/notesMaster1.xml` — single notes master shared
//!   by every notesSlide. Minimal placeholder layout.
//! - `ppt/notesSlides/notesSlideN.xml` — per-slide notes body.
//!
//! Each notesSlide references back to its slide via slide rels and
//! forward to the notesMaster via its own rels.

use crate::deck::PptxSlide;
use crate::error::Result;

/// notesMaster1.xml — minimal layout that satisfies PowerPoint's
/// expectations. PowerPoint expects each notes slide to point at this
/// master via rels.
pub fn notes_master_xml() -> Vec<u8> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:notesStyle>
    <a:lvl1pPr><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr>
  </p:notesStyle>
</p:notesMaster>
"#;
    xml.as_bytes().to_vec()
}

/// notesMaster relationships — points at the theme.
pub fn notes_master_rels() -> Result<Vec<u8>> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>
"#;
    Ok(xml.as_bytes().to_vec())
}

/// notesSlideN.xml — one per slide that has speaker_notes.
/// `slide_n` is 1-based. The body is escaped + emitted as a single
/// paragraph; multi-line notes are split on `\n` into multiple `<a:p>`.
pub fn notes_slide_xml(slide: &PptxSlide) -> Vec<u8> {
    let body = slide.speaker_notes.as_deref().unwrap_or("");
    let paragraphs = body
        .split('\n')
        .map(paragraph_xml)
        .collect::<String>();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Notes Placeholder"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="body" idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          {paragraphs}
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:notes>
"#
    );
    xml.into_bytes()
}

fn paragraph_xml(line: &str) -> String {
    if line.is_empty() {
        return "<a:p/>".to_string();
    }
    format!(
        "<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p>",
        escape_xml(line)
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// notesSlide rels — points back at its slide + at the notesMaster.
pub fn notes_slide_rels(slide_n: usize) -> Result<Vec<u8>> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide{slide_n}.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/>
</Relationships>
"#
    );
    Ok(xml.into_bytes())
}
