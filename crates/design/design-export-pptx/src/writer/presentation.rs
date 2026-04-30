use crate::deck::PptxDeck;
use crate::error::PptxWriteError;

use super::xml::{close_elem, empty_elem, into_bytes, new_writer, open_elem, write_decl};

pub fn build(deck: &PptxDeck) -> Result<Vec<u8>, PptxWriteError> {
    let mut w = new_writer();
    write_decl(&mut w)?;
    open_elem(&mut w, "p:presentation", &[
        ("xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main"),
        ("xmlns:r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
        ("xmlns:p", "http://schemas.openxmlformats.org/presentationml/2006/main"),
    ])?;

    open_elem(&mut w, "p:sldMasterIdLst", &[])?;
    empty_elem(&mut w, "p:sldMasterId", &[("id", "2147483648"), ("r:id", "rId1")])?;
    close_elem(&mut w, "p:sldMasterIdLst")?;

    open_elem(&mut w, "p:sldIdLst", &[])?;
    for i in 0..deck.slides.len() {
        let slide_id = (256 + i as u32).to_string();
        let rid = format!("rId{}", i + 3); // rId1=master, rId2=theme, rId3+ = slides
        empty_elem(&mut w, "p:sldId", &[("id", &slide_id), ("r:id", &rid)])?;
    }
    close_elem(&mut w, "p:sldIdLst")?;

    let (w_emu, h_emu) = deck
        .slides
        .first()
        .map(|s| (s.width_emu, s.height_emu))
        .unwrap_or((12_192_000, 6_858_000));
    empty_elem(&mut w, "p:sldSz", &[
        ("cx", &w_emu.to_string()),
        ("cy", &h_emu.to_string()),
    ])?;
    empty_elem(&mut w, "p:notesSz", &[("cx", "6858000"), ("cy", "9144000")])?;

    close_elem(&mut w, "p:presentation")?;
    Ok(into_bytes(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::PptxSlide;

    #[test]
    fn presentation_lists_one_slide_id_per_slide() {
        let d = PptxDeck {
            title: None,
            slides: vec![PptxSlide::blank_1280_720(), PptxSlide::blank_1280_720()],
        };
        let s = String::from_utf8(build(&d).unwrap()).unwrap();
        assert!(s.contains(r#"id="256""#));
        assert!(s.contains(r#"id="257""#));
        assert!(s.contains(r#"cx="12192000""#));
    }
}
