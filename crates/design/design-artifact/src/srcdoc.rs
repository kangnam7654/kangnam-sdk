//! Sandboxed-iframe `srcdoc` HTML wrapper.
//!
//! Wraps an artifact's raw HTML body into a self-contained document with:
//! - `<base>` so relative URLs resolve against the artifact's project folder.
//! - Console-error capture script that posts errors back to the host via
//!   `window.parent.postMessage(...)` with a known channel name.
//! - Optional `<style>` reset so the iframe doesn't inherit the host page's
//!   font / color choices.
//!
//! The Rust crate emits the wrapped HTML *string*; the actual iframe and
//! the JS bridge that consumes the postMessage events live in the
//! frontend (Tauri webview, `src/renderer/`).
//!
//! Distilled from open-design [`runtime/srcdoc.ts`][s] + open-codesign
//! `packages/runtime/src/iframe-errors.ts` (Apache-2.0 / MIT).
//!
//! [s]: https://github.com/nexu-io/open-design/blob/main/src/runtime/srcdoc.ts

#[derive(Debug, Clone, Default)]
pub struct SrcdocOpts {
    /// `<base href>` prefix (e.g. a Tauri asset-protocol URL or `/.od/<id>/`).
    /// When set, relative URLs in the artifact resolve here.
    pub base_href: Option<String>,
    /// Console-message channel name posted back to the host
    /// (`window.parent.postMessage({ channel, … })`). Default
    /// `"design-artifact-iframe"`.
    pub error_channel: Option<String>,
    /// When `true`, inject a small CSS reset so the artifact starts from
    /// margin-0 / box-sizing-border-box / system font as a fallback.
    pub include_reset: bool,
    /// When the artifact body already starts with `<!doctype` or `<html`,
    /// don't wrap it — it's already a full document.
    pub passthrough_full_documents: bool,
}

/// Wrap an artifact body for sandboxed iframe rendering.
pub fn wrap_srcdoc(body: &str, opts: &SrcdocOpts) -> String {
    let trimmed = body.trim_start();
    let already_full = trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<html");
    if opts.passthrough_full_documents && already_full {
        return body.to_string();
    }

    let channel = opts
        .error_channel
        .as_deref()
        .unwrap_or("design-artifact-iframe");
    let base_tag = match opts.base_href.as_deref() {
        Some(href) => format!("<base href=\"{}\">", html_escape_attr(href)),
        None => String::new(),
    };
    let reset = if opts.include_reset {
        r#"<style>
*, *::before, *::after { box-sizing: border-box; }
html, body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; }
</style>"#
    } else {
        ""
    };
    let bridge = format!(
        r#"<script>
(function () {{
  const channel = {channel:?};
  function post(payload) {{
    try {{
      window.parent.postMessage(Object.assign({{ channel }}, payload), '*');
    }} catch (e) {{ /* sandboxed: silently drop */ }}
  }}
  window.addEventListener('error', function (ev) {{
    post({{ kind: 'error', message: String(ev.message || ev), source: ev.filename, line: ev.lineno, col: ev.colno }});
  }});
  window.addEventListener('unhandledrejection', function (ev) {{
    post({{ kind: 'unhandled-rejection', reason: String(ev.reason || '') }});
  }});
  ['log','info','warn','error','debug'].forEach(function (level) {{
    const orig = console[level];
    console[level] = function () {{
      try {{
        const parts = Array.prototype.slice.call(arguments).map(function (a) {{
          try {{ return typeof a === 'string' ? a : JSON.stringify(a); }} catch (e) {{ return String(a); }}
        }});
        post({{ kind: 'console', level, message: parts.join(' ') }});
      }} catch (e) {{}}
      try {{ orig.apply(console, arguments); }} catch (e) {{}}
    }};
  }});
  window.addEventListener('DOMContentLoaded', function () {{
    post({{ kind: 'ready' }});
  }});
}})();
</script>"#,
        channel = channel
    );

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n{base_tag}\n{reset}\n{bridge}\n</head>\n<body>\n{body}\n</body>\n</html>\n"
    )
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_partial_body_with_doctype_and_bridge() {
        let body = "<h1>Hello</h1>";
        let out = wrap_srcdoc(body, &SrcdocOpts::default());
        assert!(out.starts_with("<!doctype html>"));
        assert!(out.contains("<h1>Hello</h1>"));
        assert!(out.contains("design-artifact-iframe"));
        assert!(out.contains("addEventListener('error'"));
    }

    #[test]
    fn passthrough_full_document_when_opted_in() {
        let body = "<!doctype html><html><body>x</body></html>";
        let opts = SrcdocOpts {
            passthrough_full_documents: true,
            ..Default::default()
        };
        let out = wrap_srcdoc(body, &opts);
        assert_eq!(out, body);
    }

    #[test]
    fn wraps_full_doc_when_passthrough_off() {
        let body = "<!doctype html><html><body>x</body></html>";
        let out = wrap_srcdoc(body, &SrcdocOpts::default());
        // Body still nested in our wrapper.
        assert!(out.contains(body));
    }

    #[test]
    fn base_href_lands_in_head() {
        let opts = SrcdocOpts {
            base_href: Some("tauri://localhost/.od/abc/".into()),
            ..Default::default()
        };
        let out = wrap_srcdoc("<p>x</p>", &opts);
        assert!(out.contains("<base href=\"tauri://localhost/.od/abc/\""));
    }

    #[test]
    fn custom_error_channel_is_used() {
        let opts = SrcdocOpts {
            error_channel: Some("kangnam-preview".into()),
            ..Default::default()
        };
        let out = wrap_srcdoc("<p>x</p>", &opts);
        assert!(out.contains("\"kangnam-preview\""));
    }

    #[test]
    fn reset_only_when_requested() {
        let with = wrap_srcdoc(
            "<p>x</p>",
            &SrcdocOpts {
                include_reset: true,
                ..Default::default()
            },
        );
        let without = wrap_srcdoc("<p>x</p>", &SrcdocOpts::default());
        assert!(with.contains("box-sizing: border-box"));
        assert!(!without.contains("box-sizing: border-box"));
    }
}
