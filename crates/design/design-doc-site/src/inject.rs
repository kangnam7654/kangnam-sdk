//! Rendered HTML 위에 zone override 를 덮어씌운다. DOM 파서 없이
//! `data-edit-zone="..."` 속성을 가진 outermost element 를 찾아 교체한다.
//!
//! 전략: regex-lite 는 backreference 를 지원하지 않으므로 수동 스캐너로 구현.
//! 1. 속성 문자열 `data-edit-zone="{id}"` 위치를 찾는다.
//! 2. 해당 위치에서 왼쪽으로 스캔해 여는 `<TAG ...>` 의 시작 `<` 를 찾는다.
//! 3. 그 TAG 이름을 추출한다.
//! 4. 여는 태그의 `>` 다음부터 오른쪽으로 스캔해 `<TAG>` 중첩을 카운트하며
//!    매칭되는 `</TAG>` 를 찾는다.
//! 5. 그 범위 전체를 replacement 로 치환한다.
//!
//! Self-closing 태그 (e.g. `<img data-edit-zone="..." />`) 는 단일 태그 치환.

use std::collections::HashMap;

pub fn apply_overrides(html: &str, overrides: &HashMap<String, String>) -> String {
    if overrides.is_empty() {
        return html.to_string();
    }
    let mut out = html.to_string();
    for (zone_id, replacement) in overrides {
        if let Some(replaced) = replace_zone(&out, zone_id, replacement) {
            out = replaced;
        }
        // Zone-miss is non-fatal; callers validate zone presence beforehand
        // when they need to surface miss diagnostics.
    }
    out
}

/// v2 slide-scoped override application. Each slide's overrides are applied
/// only inside its `<section data-slide-id="<id>">...</section>` block, so
/// the same zone id on different slides stays separate.
///
/// Falls back to the flat `apply_overrides` behavior when the HTML has no
/// `data-slide-id` at all — legacy v1 versions were rendered as a single
/// slide without the section wrapper, and we still want their overrides to
/// reach the right target.
pub fn apply_overrides_by_slide(
    html: &str,
    overrides: &HashMap<String, HashMap<String, String>>,
) -> String {
    if overrides.is_empty() {
        return html.to_string();
    }

    // Legacy fallback: no slide wrappers → apply every override against
    // the full HTML as if it were one slide.
    if !html.contains("data-slide-id=") {
        let mut flat: HashMap<String, String> = HashMap::new();
        for zones in overrides.values() {
            for (zid, rep) in zones {
                flat.entry(zid.clone()).or_insert_with(|| rep.clone());
            }
        }
        return apply_overrides(html, &flat);
    }

    let mut out = html.to_string();
    for (slide_id, zones) in overrides {
        if zones.is_empty() {
            continue;
        }
        let Some((section_start, section_end)) = find_slide_section(&out, slide_id) else {
            // Unknown slide id — no matching section. Skip rather than
            // corrupt other slides; callers pre-validate when they need
            // miss diagnostics.
            continue;
        };

        let section_body = &out[section_start..section_end];
        let replaced_body = apply_overrides(section_body, zones);
        if replaced_body == section_body {
            continue;
        }
        let mut next = String::with_capacity(out.len() + replaced_body.len());
        next.push_str(&out[..section_start]);
        next.push_str(&replaced_body);
        next.push_str(&out[section_end..]);
        out = next;
    }
    out
}

/// Return `(start, end)` byte offsets of the `<section ... data-slide-id="<id>"
/// ...>...</section>` block for the requested slide id. `start` points at the
/// opening `<section`; `end` is the offset just past the closing `</section>`.
/// Supports nested `<section>` children by depth counting.
fn find_slide_section(html: &str, slide_id: &str) -> Option<(usize, usize)> {
    let needle = format!(r#"data-slide-id="{}""#, slide_id);
    let attr_pos = html.find(&needle)?;

    let before = &html[..attr_pos];
    let open_start = before.rfind("<section").filter(|&i| {
        // Must be a genuine `<section` — next char after "section" is
        // whitespace or `>`, not a letter.
        let after = &html[i + "<section".len()..];
        after
            .chars()
            .next()
            .map(|c| c.is_whitespace() || c == '>')
            .unwrap_or(false)
    })?;
    // Ensure the `<section` we found is the one carrying our attribute
    // (no `>` between them).
    if html[open_start..attr_pos].contains('>') {
        return None;
    }

    let open_gt = html[attr_pos..].find('>').map(|i| attr_pos + i)?;
    let body_start = open_gt + 1;

    let close_tag = "</section>";
    let open_tag_prefix_with_space = "<section ";
    let open_tag_prefix_with_gt = "<section>";

    let mut depth: usize = 0;
    let mut cursor = body_start;
    loop {
        let rest = &html[cursor..];
        let next_close = rest.find(close_tag).map(|i| cursor + i);
        let next_open_space = rest.find(open_tag_prefix_with_space).map(|i| cursor + i);
        let next_open_gt = rest.find(open_tag_prefix_with_gt).map(|i| cursor + i);

        let next_open = match (next_open_space, next_open_gt) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        match (next_close, next_open) {
            (None, _) => return None,
            (Some(c), Some(o)) if o < c => {
                depth += 1;
                cursor = o + open_tag_prefix_with_space.len();
            }
            (Some(c), _) => {
                if depth == 0 {
                    return Some((open_start, c + close_tag.len()));
                } else {
                    depth -= 1;
                    cursor = c + close_tag.len();
                }
            }
        }
    }
}

/// zone_id 에 매칭되는 outermost element 를 replacement 로 교체. 못 찾으면 None.
fn replace_zone(html: &str, zone_id: &str, replacement: &str) -> Option<String> {
    let needle = format!(r#"data-edit-zone="{}""#, zone_id);
    let attr_pos = html.find(&needle)?;

    // 왼쪽으로 스캔: 속성 위치 이전에서 가장 가까운 `<` 를 찾는다.
    let before = &html[..attr_pos];
    let open_start = before.rfind('<')?;
    if html[open_start..attr_pos].contains('>') {
        return None;
    }

    // 태그 이름: `<` 다음부터 공백 또는 `>` 전까지.
    let name_start = open_start + 1;
    let name_end = html[name_start..]
        .find(|c: char| c.is_whitespace() || c == '>')
        .map(|i| name_start + i)?;
    let tag_name = &html[name_start..name_end];
    if tag_name.is_empty() {
        return None;
    }

    let open_gt = html[attr_pos..].find('>').map(|i| attr_pos + i)?;

    // self-closing.
    if html[attr_pos..open_gt].trim_end().ends_with('/') {
        let mut out = String::with_capacity(html.len() + replacement.len());
        out.push_str(&html[..open_start]);
        out.push_str(replacement);
        out.push_str(&html[open_gt + 1..]);
        return Some(out);
    }

    let body_start = open_gt + 1;
    let close_tag = format!("</{}>", tag_name);
    let open_tag_prefix_with_space = format!("<{} ", tag_name);
    let open_tag_prefix_with_gt = format!("<{}>", tag_name);

    let mut depth: usize = 0;
    let mut cursor = body_start;
    let body_end;
    loop {
        let rest = &html[cursor..];
        let next_close = rest.find(&close_tag).map(|i| cursor + i);
        let next_open_space = rest.find(&open_tag_prefix_with_space).map(|i| cursor + i);
        let next_open_gt = rest.find(&open_tag_prefix_with_gt).map(|i| cursor + i);

        let next_open = match (next_open_space, next_open_gt) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        match (next_close, next_open) {
            (None, _) => return None,
            (Some(c), Some(o)) if o < c => {
                depth += 1;
                cursor = o + open_tag_prefix_with_space.len();
            }
            (Some(c), _) => {
                if depth == 0 {
                    body_end = c;
                    break;
                } else {
                    depth -= 1;
                    cursor = c + close_tag.len();
                }
            }
        }
    }

    let close_tag_end = body_end + close_tag.len();
    let mut out = String::with_capacity(html.len() + replacement.len());
    out.push_str(&html[..open_start]);
    out.push_str(replacement);
    out.push_str(&html[close_tag_end..]);
    Some(out)
}

/// zone walker 에 필요한 zone 메타데이터를 script tag 로 주입.
/// body 닫기 직전에 삽입; body 없으면 맨 뒤에 append.
pub fn inject_zone_meta(html: &str, zones_json: &str) -> String {
    let script = format!(
        r#"<script type="application/json" id="__edit_zones__">{}</script>"#,
        zones_json
    );
    if let Some(idx) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + script.len());
        out.push_str(&html[..idx]);
        out.push_str(&script);
        out.push_str(&html[idx..]);
        out
    } else {
        format!("{}{}", html, script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn replaces_single_zone() {
        let html =
            r#"<html><body><div data-edit-zone="cover.tagline" class="t">old</div></body></html>"#;
        let out = apply_overrides(
            html,
            &map(&[(
                "cover.tagline",
                "<div data-edit-zone=\"cover.tagline\" class=\"t\">new</div>",
            )]),
        );
        assert!(out.contains("new"), "replacement missing: {out}");
        assert!(!out.contains(">old<"), "old content still present: {out}");
    }

    #[test]
    fn noop_when_empty_overrides() {
        let html = "<div>stuff</div>";
        assert_eq!(apply_overrides(html, &HashMap::new()), html);
    }

    #[test]
    fn missing_zone_leaves_html_untouched() {
        let html = r#"<div data-edit-zone="a">x</div>"#;
        let out = apply_overrides(html, &map(&[("b", "<div data-edit-zone=\"b\">y</div>")]));
        assert_eq!(out, html);
    }

    #[test]
    fn replaces_only_first_occurrence_same_id() {
        let html = r#"<div data-edit-zone="z">a</div><div data-edit-zone="z">b</div>"#;
        let out = apply_overrides(html, &map(&[("z", "<div data-edit-zone=\"z\">X</div>")]));
        assert!(out.starts_with(r#"<div data-edit-zone="z">X</div>"#));
        assert!(out.contains(">b</div>"));
    }

    #[test]
    fn inject_zone_meta_places_before_body_close() {
        let html = "<html><body>hi</body></html>";
        let out = inject_zone_meta(html, r#"{"a":1}"#);
        assert!(out.contains(
            r#"<script type="application/json" id="__edit_zones__">{"a":1}</script></body>"#
        ));
    }

    #[test]
    fn inject_zone_meta_appends_when_no_body() {
        let out = inject_zone_meta("<p>x</p>", "{}");
        assert!(out.ends_with(r#"</script>"#));
    }

    #[test]
    fn handles_nested_children() {
        let html = r#"<section data-edit-zone="exp.0"><h3>t</h3><p>body</p></section>"#;
        let replacement = r#"<section data-edit-zone="exp.0"><h3>new-t</h3></section>"#;
        let out = apply_overrides(html, &map(&[("exp.0", replacement)]));
        assert!(out.contains("new-t"));
        assert!(!out.contains("body"));
    }

    #[test]
    fn handles_nested_same_tag() {
        let html = r#"<div data-edit-zone="z"><div>inner</div></div>"#;
        let replacement = r#"<div data-edit-zone="z"><span>new</span></div>"#;
        let out = apply_overrides(html, &map(&[("z", replacement)]));
        assert_eq!(out, replacement);
    }

    #[test]
    fn handles_self_closing_tag() {
        let html = r#"<div><img data-edit-zone="img.0" src="a.png" /></div>"#;
        let replacement = r#"<img data-edit-zone="img.0" src="b.png" />"#;
        let out = apply_overrides(html, &map(&[("img.0", replacement)]));
        assert!(out.contains("b.png"));
        assert!(!out.contains("a.png"));
    }

    #[test]
    fn preserves_content_around_zone() {
        let html = r#"before<p data-edit-zone="z">mid</p>after"#;
        let replacement = r#"<p data-edit-zone="z">MID</p>"#;
        let out = apply_overrides(html, &map(&[("z", replacement)]));
        assert_eq!(out, "before<p data-edit-zone=\"z\">MID</p>after");
    }

    #[test]
    fn multiple_zones_all_applied() {
        let html = r#"<p data-edit-zone="a">A</p><p data-edit-zone="b">B</p>"#;
        let overrides = map(&[
            ("a", r#"<p data-edit-zone="a">A2</p>"#),
            ("b", r#"<p data-edit-zone="b">B2</p>"#),
        ]);
        let out = apply_overrides(html, &overrides);
        assert!(out.contains(">A2<"));
        assert!(out.contains(">B2<"));
    }

    // ----- P5.5: slide-scoped overrides -----

    fn slide_map(entries: &[(&str, &[(&str, &str)])]) -> HashMap<String, HashMap<String, String>> {
        entries
            .iter()
            .map(|(sid, zs)| {
                (
                    sid.to_string(),
                    zs.iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn apply_overrides_by_slide_scopes_to_matching_section() {
        // Two slides, same zone id ("title"). Without slide-scoping the
        // first replacement would consume slide 1 correctly but then the
        // second would find slide 2's title intact and replace it with
        // slide 1's replacement — users would see the wrong title on
        // slide 2. With slide-scoping each replacement is confined to
        // its `<section data-slide-id="...">` block.
        let html = concat!(
            r#"<section data-slide-id="s1" class="slide"><h1 data-edit-zone="title">A1</h1></section>"#,
            r#"<section data-slide-id="s2" class="slide"><h1 data-edit-zone="title">B1</h1></section>"#,
        );
        let overrides = slide_map(&[
            ("s1", &[("title", r#"<h1 data-edit-zone="title">A2</h1>"#)]),
            ("s2", &[("title", r#"<h1 data-edit-zone="title">B2</h1>"#)]),
        ]);
        let out = apply_overrides_by_slide(html, &overrides);
        assert!(out.contains(">A2<"), "slide 1 title replaced: {out}");
        assert!(out.contains(">B2<"), "slide 2 title replaced: {out}");
        assert!(!out.contains(">A1<"));
        assert!(!out.contains(">B1<"));
    }

    #[test]
    fn apply_overrides_by_slide_legacy_html_without_sections_falls_back() {
        // Legacy v1 HTML (stored pre-Deck rendering) has no `<section
        // data-slide-id>` wrapper. Slide-scoped overrides must still be
        // applied — treat the whole HTML as one slide and apply every
        // slide's overrides in one pass.
        let html = r#"<div data-edit-zone="title">원본</div>"#;
        let overrides = slide_map(&[(
            "s1",
            &[("title", r#"<div data-edit-zone="title">새</div>"#)],
        )]);
        let out = apply_overrides_by_slide(html, &overrides);
        assert!(
            out.contains("새"),
            "legacy HTML still gets overridden: {out}"
        );
        assert!(!out.contains("원본"));
    }

    #[test]
    fn apply_overrides_by_slide_ignores_unknown_slide_ids() {
        // Override targets a slide that isn't present. Don't crash; leave
        // the HTML as-is for the unmatched slide, apply known slides.
        let html = r#"<section data-slide-id="s1" class="slide"><h1 data-edit-zone="title">A</h1></section>"#;
        let overrides = slide_map(&[
            ("s1", &[("title", r#"<h1 data-edit-zone="title">A2</h1>"#)]),
            (
                "s9",
                &[("title", r#"<h1 data-edit-zone="title">ghost</h1>"#)],
            ),
        ]);
        let out = apply_overrides_by_slide(html, &overrides);
        assert!(out.contains(">A2<"));
        assert!(!out.contains("ghost"));
    }

    // Canvas-context regression: works on real `to_html::render` output.
    #[test]
    fn replaces_zone_in_canvas_rendered_html() {
        use crate::html_render as to_html;
        use kangnam_design_doc_slide::slide::{Frame, SlideDoc, SlideElement, TextStyle};

        let mut doc = SlideDoc::empty("s1");
        doc.elements.push(SlideElement::Text {
            id: "title".into(),
            frame: Frame {
                x: 80.0,
                y: 300.0,
                w: 1120.0,
                h: 120.0,
            },
            content: "원본 제목".into(),
            style: TextStyle::default(),
        });
        let rendered = to_html::render(&doc);
        assert!(rendered.contains("원본 제목"));

        let new_html = r#"<div data-edit-zone="title" data-edit-label="title" style="left:80.00px;top:300.00px;">새 제목</div>"#;
        let out = apply_overrides(&rendered, &map(&[("title", new_html)]));

        assert!(out.contains("새 제목"), "new content missing: {out}");
        assert!(!out.contains("원본 제목"), "old content still present");
        assert!(
            out.contains("data-slide-id=\"s1\""),
            "slide wrapper preserved"
        );
    }
}
