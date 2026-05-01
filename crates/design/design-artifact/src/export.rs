//! Artifact export — convert an artifact body into HTML / Markdown /
//! ZIP payloads ready for download.
//!
//! Phase 5c-16 shipped the lightweight exports (HTML pass-through,
//! Markdown). Phase 6a extends with:
//! - `export_html_inline(body, assets)` — inlines referenced assets as
//!   data URIs so the downloaded HTML is fully self-contained
//! - `export_zip(entries)` — bundles a multi-file artifact (HTML +
//!   referenced assets) into a single .zip download
//!
//! PDF export lives entirely in the Tauri webview — the renderer
//! triggers `window.print()` on the artifact iframe and the OS print
//! dialog handles "Save as PDF". No Rust path needed for that.
//!
//! All functions are pure: input is the raw artifact body string,
//! output is the format-specific bytes. The host wraps them in Tauri
//! commands (Phase 5c-17) that surface to the renderer chips.

/// Produce an HTML download — the artifact body is already HTML in
/// the common case, so this is largely a pass-through. We do wrap in
/// a minimal `<!doctype html>` shell when the body doesn't already
/// declare one, so the downloaded file opens directly in a browser.
pub fn export_html(body: &str) -> String {
    let trimmed = body.trim_start();
    if trimmed
        .to_ascii_lowercase()
        .starts_with("<!doctype")
        || trimmed.to_ascii_lowercase().starts_with("<html")
    {
        return body.to_string();
    }
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\" />\n<title>Artifact</title>\n</head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

/// Produce a Markdown export. The HTML body is reduced to a minimal
/// markdown via heuristic stripping (drop `<style>`, `<script>` and
/// `<head>` blocks; replace `<br>` with newlines; replace common
/// block-level tags with their text content).
///
/// Intentionally simple — for high-fidelity Markdown the user should
/// pick HTML and convert externally. This export targets the case of
/// "I want a quick text snapshot".
pub fn export_markdown(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    let mut in_tag = false;
    let lower = body.to_ascii_lowercase();

    // Find skip-block ranges so we don't include their text.
    let skip_ranges = find_block_ranges(&lower, &["style", "script", "head"]);

    let mut idx = 0usize;
    while let Some(c) = chars.next() {
        let pos = idx;
        idx += c.len_utf8();
        // Honor skip ranges over the original byte offsets.
        if let Some((_, end)) = skip_ranges.iter().find(|(s, e)| *s <= pos && pos < *e) {
            // Fast-forward chars until we pass `end`.
            while idx < *end {
                if let Some(nc) = chars.next() {
                    idx += nc.len_utf8();
                } else {
                    break;
                }
            }
            continue;
        }
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            // Collapse runs of whitespace introduced by tag boundaries.
            if !out.ends_with(char::is_whitespace) {
                out.push(' ');
            }
            continue;
        }
        if !in_tag {
            out.push(c);
        }
    }

    // Collapse multiple blank lines.
    let mut collapsed = String::with_capacity(out.len());
    let mut blank_run = 0usize;
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                collapsed.push('\n');
            }
        } else {
            blank_run = 0;
            collapsed.push_str(t);
            collapsed.push('\n');
        }
    }
    collapsed
}

fn find_block_ranges(haystack: &str, tags: &[&str]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for tag in tags {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        let mut start = 0usize;
        while let Some(open_idx_rel) = haystack[start..].find(&open) {
            let open_idx = start + open_idx_rel;
            // Find the closing tag.
            let close_idx = match haystack[open_idx..].find(&close) {
                Some(i) => open_idx + i + close.len(),
                None => break,
            };
            out.push((open_idx, close_idx));
            start = close_idx;
        }
    }
    out.sort_by_key(|r| r.0);
    out
}

/// A named asset to inline into HTML or pack into a ZIP archive.
/// `mime` is used for data URI generation (HTML inline) and is also
/// stored in the ZIP comment for round-trip awareness.
#[derive(Debug, Clone)]
pub struct Asset {
    /// POSIX-style relative path (e.g. `assets/hero.png`).
    pub path: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Inline every asset URL appearing in the body's `src=` / `href=`
/// attributes that matches a key in the `assets` map. The replacement
/// is a `data:<mime>;base64,<...>` URI so the resulting HTML opens
/// directly without filesystem access.
///
/// Unmatched asset URLs are left untouched (e.g. `https://...` external
/// CDN refs). Quoted attribute values (single or double quotes) are
/// both supported.
pub fn export_html_inline(body: &str, assets: &[Asset]) -> String {
    use base64::Engine as _;
    let mut out = body.to_string();
    for a in assets {
        let data_uri = format!(
            "data:{};base64,{}",
            a.mime,
            base64::engine::general_purpose::STANDARD.encode(&a.bytes)
        );
        // Replace exact attribute values both with and without leading `./`.
        let candidates = [
            format!("\"{}\"", a.path),
            format!("'{}'", a.path),
            format!("\"./{}\"", a.path),
            format!("'./{}'", a.path),
        ];
        let replacements = [
            format!("\"{}\"", data_uri),
            format!("'{}'", data_uri),
            format!("\"{}\"", data_uri),
            format!("'{}'", data_uri),
        ];
        for (needle, replace) in candidates.iter().zip(replacements.iter()) {
            out = out.replace(needle.as_str(), replace.as_str());
        }
    }
    out
}

/// Build a ZIP archive containing the artifact's HTML body + all
/// referenced assets. Returns the raw .zip bytes.
///
/// The HTML body is written as `index.html`; assets land at their
/// declared relative paths.
pub fn export_zip(html: &str, assets: &[Asset]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    let buf = Vec::new();
    let mut cursor = std::io::Cursor::new(buf);
    {
        let mut w = zip::ZipWriter::new(&mut cursor);
        let opts: SimpleFileOptions =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        w.start_file("index.html", opts).map_err(|e| e.to_string())?;
        w.write_all(html.as_bytes()).map_err(|e| e.to_string())?;
        for a in assets {
            w.start_file(&a.path, opts).map_err(|e| e.to_string())?;
            w.write_all(&a.bytes).map_err(|e| e.to_string())?;
        }
        w.finish().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_pass_through_when_doctype_present() {
        let body = "<!doctype html><html><body>hi</body></html>";
        let out = export_html(body);
        assert_eq!(out, body);
    }

    #[test]
    fn html_wrapped_when_fragment() {
        let body = "<p>hi</p>";
        let out = export_html(body);
        assert!(out.starts_with("<!doctype html>"));
        assert!(out.contains("<p>hi</p>"));
    }

    #[test]
    fn markdown_strips_tags() {
        let body = "<h1>Title</h1><p>Body <strong>bold</strong> text.</p>";
        let out = export_markdown(body);
        assert!(out.contains("Title"));
        assert!(out.contains("Body"));
        assert!(out.contains("bold"));
        assert!(!out.contains("<h1>"));
    }

    #[test]
    fn markdown_drops_script_and_style() {
        let body = "<style>a{color:red}</style><p>visible</p><script>alert(1)</script>";
        let out = export_markdown(body);
        assert!(out.contains("visible"));
        assert!(!out.contains("color:red"));
        assert!(!out.contains("alert"));
    }

    #[test]
    fn html_inline_double_quoted_src() {
        let body = "<img src=\"hero.png\">";
        let assets = vec![Asset {
            path: "hero.png".into(),
            mime: "image/png".into(),
            bytes: vec![1, 2, 3],
        }];
        let out = export_html_inline(body, &assets);
        assert!(out.contains("data:image/png;base64,"));
        assert!(!out.contains("\"hero.png\""));
    }

    #[test]
    fn html_inline_single_quoted_src() {
        let body = "<img src='./logo.svg'>";
        let assets = vec![Asset {
            path: "logo.svg".into(),
            mime: "image/svg+xml".into(),
            bytes: b"<svg/>".to_vec(),
        }];
        let out = export_html_inline(body, &assets);
        assert!(out.contains("data:image/svg+xml;base64,"));
        assert!(!out.contains("'./logo.svg'"));
    }

    #[test]
    fn html_inline_leaves_external_urls_alone() {
        let body = "<img src=\"https://example.com/x.png\">";
        let assets: Vec<Asset> = vec![];
        let out = export_html_inline(body, &assets);
        assert_eq!(out, body);
    }

    #[test]
    fn zip_contains_index_and_assets() {
        let html = "<p>hi</p>";
        let assets = vec![Asset {
            path: "assets/cover.png".into(),
            mime: "image/png".into(),
            bytes: vec![0xff, 0xee],
        }];
        let bytes = export_zip(html, &assets).expect("zip ok");
        // Read back via zip::ZipArchive to verify entries.
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).expect("read zip");
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"index.html".to_string()));
        assert!(names.contains(&"assets/cover.png".to_string()));
    }
}
