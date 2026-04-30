use design_doc_slide::slide::{
    Background, Fill, Frame, ImageFit, ShapeKind, SlideDoc, SlideElement, Stroke, TextAlign,
    TextStyle,
};

const ZONE_ATTR: &str = "data-edit-zone";
const LABEL_ATTR: &str = "data-edit-label";

/// Render `SlideDoc` as a standalone HTML document.
/// Each [`SlideElement`] gets a `data-edit-zone="<id>"` attribute so the zone-walker
/// can pick it up.
pub fn render(doc: &SlideDoc) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str("<!DOCTYPE html>\n");
    out.push_str("<html lang=\"ko\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=1280\">\n");
    out.push_str("<title>Canvas Slide</title>\n");
    out.push_str("<style>\n");
    out.push_str(&base_css(doc));
    out.push_str("</style>\n</head>\n<body>\n");

    out.push_str(&format!(
        "<div class=\"canvas-slide\" data-slide-id=\"{}\">\n",
        escape_html(&doc.id)
    ));

    for el in &doc.elements {
        out.push_str(&render_element(el));
        out.push('\n');
    }

    out.push_str("</div>\n</body>\n</html>\n");
    out
}

/// Render a single slide as a `<section class="slide">` block suitable for
/// stitching multiple slides into a Deck HTML document. The section carries
/// `data-slide-id` (stable UUID) and `data-slide-index` (DOM order) for the
/// navigator + override compositor. Background is inlined on the section so
/// each slide can carry its own style without a stylesheet-per-slide.
pub fn render_section(doc: &SlideDoc, index: usize) -> String {
    let mut out = String::with_capacity(1024);
    let bg = render_background_css(&doc.background);
    out.push_str(&format!(
        "<section class=\"slide\" data-slide-id=\"{id}\" data-slide-index=\"{idx}\" style=\"{bg}\">\n",
        id = escape_attr(&doc.id),
        idx = index,
        bg = bg,
    ));
    for el in &doc.elements {
        out.push_str(&render_element(el));
        out.push('\n');
    }
    out.push_str("</section>");
    out
}

fn base_css(doc: &SlideDoc) -> String {
    let bg = render_background_css(&doc.background);
    format!(
        r#"
html, body {{ margin: 0; padding: 0; background: #f5f5f5; font-family: 'Pretendard', system-ui, -apple-system, sans-serif; }}
* {{ box-sizing: border-box; }}
.canvas-slide {{
  position: relative;
  width: {w}px;
  height: {h}px;
  margin: 0 auto;
  overflow: hidden;
  {bg}
}}
.canvas-slide [{zone}] {{ position: absolute; }}
"#,
        w = doc.width_px,
        h = doc.height_px,
        bg = bg,
        zone = ZONE_ATTR,
    )
}

fn render_background_css(bg: &Background) -> String {
    match bg {
        Background::Color { color } => format!("background: {};", escape_css(color)),
        Background::Gradient { from, to, angle_deg } => format!(
            "background: linear-gradient({deg}deg, {from} 0%, {to} 100%);",
            deg = angle_deg,
            from = escape_css(from),
            to = escape_css(to),
        ),
        Background::Image { src } => format!(
            "background: url('{}') center/cover no-repeat;",
            escape_css(src)
        ),
    }
}

fn render_element(el: &SlideElement) -> String {
    let frame = el.frame();
    let pos = format!(
        "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;",
        frame.x, frame.y, frame.w, frame.h
    );

    match el {
        SlideElement::Text { id, content, style, .. } => {
            let style_css = text_style_css(style);
            format!(
                r#"<div {zone}="{id}" {label}="{id}" style="{pos}{style_css}">{content}</div>"#,
                zone = ZONE_ATTR,
                label = LABEL_ATTR,
                id = escape_attr(id),
                pos = pos,
                style_css = style_css,
                content = escape_html(content),
            )
        }
        SlideElement::Shape { id, shape, fill, stroke, .. } => {
            let mut css = pos.clone();
            css.push_str(&fill_css(fill));
            if let Some(s) = stroke {
                css.push_str(&stroke_css(s));
            }
            css.push_str(&shape_css(*shape, &el.frame()));
            format!(
                r#"<div {zone}="{id}" {label}="{id}" style="{css}"></div>"#,
                zone = ZONE_ATTR,
                label = LABEL_ATTR,
                id = escape_attr(id),
                css = css,
            )
        }
        SlideElement::Image { id, src, fit, .. } => {
            let object_fit = match fit {
                ImageFit::Cover => "cover",
                ImageFit::Contain => "contain",
                ImageFit::Fill => "fill",
            };
            format!(
                r#"<img {zone}="{id}" {label}="{id}" src="{src}" style="{pos}object-fit:{fit};" alt="">"#,
                zone = ZONE_ATTR,
                label = LABEL_ATTR,
                id = escape_attr(id),
                src = escape_attr(src),
                pos = pos,
                fit = object_fit,
            )
        }
    }
}

fn text_style_css(s: &TextStyle) -> String {
    let align = match s.align {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
        TextAlign::Justify => "justify",
    };
    format!(
        "font-family:'{ff}',sans-serif;font-size:{fs}px;font-weight:{fw};color:{c};line-height:{lh};letter-spacing:{ls}px;text-align:{al};white-space:pre-wrap;",
        ff = escape_css(&s.font_family),
        fs = s.font_size_px,
        fw = s.font_weight,
        c = escape_css(&s.color),
        lh = s.line_height,
        ls = s.letter_spacing_px,
        al = align,
    )
}

fn fill_css(fill: &Fill) -> String {
    match fill {
        Fill::Solid { color } => format!("background:{};", escape_css(color)),
        Fill::Gradient { from, to, angle_deg } => format!(
            "background:linear-gradient({deg}deg,{from} 0%,{to} 100%);",
            deg = angle_deg,
            from = escape_css(from),
            to = escape_css(to),
        ),
        Fill::None => String::new(),
    }
}

fn stroke_css(s: &Stroke) -> String {
    format!("border:{}px solid {};", s.width_px, escape_css(&s.color))
}

fn shape_css(shape: ShapeKind, frame: &Frame) -> String {
    match shape {
        ShapeKind::Rect => String::new(),
        ShapeKind::RoundedRect => "border-radius:12px;".to_string(),
        ShapeKind::Circle => format!("border-radius:{}px;", (frame.w.min(frame.h)) / 2.0),
        ShapeKind::Line => "height:1px;".to_string(),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// CSS values: strip newlines and `}` to prevent breaking out of declarations.
fn escape_css(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\n' | '\r' | '}' | '{' | ';' | '"' | '\''))
        .collect()
}

#[cfg(test)]
mod tests {
    use design_doc_slide::slide::*;
    use super::*;

    #[test]
    fn empty_doc_renders_canvas_dimensions() {
        let html = render(&SlideDoc::empty("s1"));
        assert!(html.contains("width: 1280px"));
        assert!(html.contains("height: 720px"));
        assert!(html.contains("Pretendard"));
        assert!(html.contains("data-slide-id=\"s1\""));
    }

    #[test]
    fn each_element_carries_zone_attribute() {
        let mut doc = SlideDoc::empty("s2");
        doc.elements.push(SlideElement::Text {
            id: "title".into(),
            frame: Frame { x: 0.0, y: 0.0, w: 100.0, h: 100.0 },
            content: "hi".into(),
            style: TextStyle::default(),
        });
        doc.elements.push(SlideElement::Shape {
            id: "bar".into(),
            frame: Frame { x: 0.0, y: 100.0, w: 100.0, h: 10.0 },
            shape: ShapeKind::Rect,
            fill: Fill::solid("#000"),
            stroke: None,
        });

        let html = render(&doc);
        assert!(html.contains(r#"data-edit-zone="title""#));
        assert!(html.contains(r#"data-edit-zone="bar""#));
        assert_eq!(
            html.matches("data-edit-zone=").count(),
            2,
            "exactly one data-edit-zone per element"
        );
    }

    #[test]
    fn text_content_is_html_escaped() {
        let mut doc = SlideDoc::empty("s3");
        doc.elements.push(SlideElement::Text {
            id: "x".into(),
            frame: Frame { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
            content: "<script>alert(1)</script>".into(),
            style: TextStyle::default(),
        });
        let html = render(&doc);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn gradient_background_emits_linear_gradient() {
        let mut doc = SlideDoc::empty("s4");
        doc.background = Background::Gradient {
            from: "#000".into(),
            to: "#fff".into(),
            angle_deg: 90.0,
        };
        let html = render(&doc);
        assert!(html.contains("linear-gradient(90deg, #000 0%, #fff 100%)"), "got: {html}");
    }

    #[test]
    fn shape_circle_uses_border_radius_half_min_dim() {
        let mut doc = SlideDoc::empty("s5");
        doc.elements.push(SlideElement::Shape {
            id: "dot".into(),
            frame: Frame { x: 0.0, y: 0.0, w: 40.0, h: 80.0 },
            shape: ShapeKind::Circle,
            fill: Fill::solid("#f00"),
            stroke: None,
        });
        let html = render(&doc);
        assert!(html.contains("border-radius:20px"));
    }
}
