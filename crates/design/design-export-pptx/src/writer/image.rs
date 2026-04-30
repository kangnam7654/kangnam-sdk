use crate::element::{ImageBox, ImageFit, ImageMime};
use crate::error::PptxWriteError;

use super::xml::{close_elem, empty_elem, open_elem, XmlWriter};

/// Verify the caller-declared `mime` matches the first few bytes of `bytes`.
pub fn check_mime(bytes: &[u8], declared: ImageMime) -> Result<(), PptxWriteError> {
    let detected = sniff(bytes);
    match (declared, detected) {
        (ImageMime::Png, Some(ImageMime::Png)) => Ok(()),
        (ImageMime::Jpeg, Some(ImageMime::Jpeg)) => Ok(()),
        (declared, Some(actual)) if declared != actual => {
            Err(PptxWriteError::MimeMismatch {
                declared: declared.content_type(),
                detected: actual.content_type(),
            })
        }
        (declared, None) => Err(PptxWriteError::MimeMismatch {
            declared: declared.content_type(),
            detected: "unknown",
        }),
        _ => Ok(()),
    }
}

fn sniff(bytes: &[u8]) -> Option<ImageMime> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Some(ImageMime::Png); }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(ImageMime::Jpeg);
    }
    None
}

/// Emit `<p:pic>` for one image. `rel_id` is the relationship id of the
/// image in this slide's rels file (e.g. "rId2").
pub fn emit_pic(
    w: &mut XmlWriter,
    img: &ImageBox,
    sp_id: u32,
    rel_id: &str,
) -> Result<(), PptxWriteError> {
    open_elem(w, "p:pic", &[])?;

    open_elem(w, "p:nvPicPr", &[])?;
    empty_elem(w, "p:cNvPr", &[("id", &sp_id.to_string()), ("name", &format!("Picture {sp_id}"))])?;
    empty_elem(w, "p:cNvPicPr", &[])?;
    empty_elem(w, "p:nvPr", &[])?;
    close_elem(w, "p:nvPicPr")?;

    open_elem(w, "p:blipFill", &[])?;
    empty_elem(w, "a:blip", &[("r:embed", rel_id)])?;
    match img.fit {
        ImageFit::Cover | ImageFit::Contain => { empty_elem(w, "a:stretch", &[])?; }
        ImageFit::Fill => {
            open_elem(w, "a:stretch", &[])?;
            empty_elem(w, "a:fillRect", &[])?;
            close_elem(w, "a:stretch")?;
        }
    }
    close_elem(w, "p:blipFill")?;

    open_elem(w, "p:spPr", &[])?;
    open_elem(w, "a:xfrm", &[])?;
    empty_elem(w, "a:off", &[
        ("x", &img.frame.x_emu.to_string()),
        ("y", &img.frame.y_emu.to_string()),
    ])?;
    empty_elem(w, "a:ext", &[
        ("cx", &img.frame.w_emu.to_string()),
        ("cy", &img.frame.h_emu.to_string()),
    ])?;
    close_elem(w, "a:xfrm")?;
    open_elem(w, "a:prstGeom", &[("prst", "rect")])?;
    empty_elem(w, "a:avLst", &[])?;
    close_elem(w, "a:prstGeom")?;
    close_elem(w, "p:spPr")?;

    close_elem(w, "p:pic")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sniff_png_magic() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n"), Some(ImageMime::Png));
    }
    #[test]
    fn sniff_jpeg_magic() {
        assert_eq!(sniff(b"\xFF\xD8\xFFthere"), Some(ImageMime::Jpeg));
    }
    #[test]
    fn sniff_unknown() {
        assert_eq!(sniff(b"xxxx"), None);
    }
}
