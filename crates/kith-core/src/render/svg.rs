//! The SVG body — the same vocabulary `TreeCanvas`/`TreeNode` draw, on the Rust
//! side. Reads `node.x/y/width/height` and `link.anchors`; computes only the
//! `viewBox` (from `bounds`) and the rounded-elbow stroke between the model's own
//! waypoints. No node is positioned, no link is re-routed.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::escape::escape;
use crate::layout::metrics::{PORTRAIT_D, PORTRAIT_INSET};
use crate::layout::{LayoutLink, LayoutModel, LayoutNode, LinkKind, NodeKind, Point, Rect};
use crate::model::{MediaId, Sex};

/// Portrait URLs to embed, or `None` to draw no portraits. `Some(map)` carries
/// the caller-resolved `MediaId` → `data:` URL map (the resolver runs over the
/// *redacted* model, so a living person's id is already absent).
type Portraits<'a> = Option<&'a BTreeMap<MediaId, String>>;

/// Viewport breathing room, mirroring the canvas `MARGIN` (TreeCanvas).
const MARGIN: f64 = 48.0;
/// Rounded-elbow radius over the model's anchors, mirroring the canvas `CORNER`.
const CORNER: f64 = 10.0;

/// Format an emitted SVG coordinate: round to 3 decimals, strip trailing zeros and
/// a bare `.0`, and normalise `-0` to `0`. Keeps snapshots clean and the
/// determinism guarantee robust to float-formatting noise from the elbow math.
fn fmt_coord(v: f64) -> String {
    let mut s = format!("{v:.3}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s.clear();
        s.push('0');
    }
    s
}

/// `viewBox` = the tight `bounds` expanded by `MARGIN` on every side (the Rust
/// mirror of `viewport.ts::viewBoxFor`).
fn view_box(bounds: Rect) -> String {
    format!(
        "{} {} {} {}",
        fmt_coord(bounds.x - MARGIN),
        fmt_coord(bounds.y - MARGIN),
        fmt_coord(bounds.width + MARGIN * 2.0),
        fmt_coord(bounds.height + MARGIN * 2.0),
    )
}

/// Append the `<svg> … </svg>` body: the `<defs>` shadow filter and the single
/// root `<g class="root">` (the pan/zoom transform target), drawing **links then
/// nodes** in model order — the canvas's stacking.
pub(crate) fn write_svg(buf: &mut String, model: &LayoutModel, portraits: Portraits<'_>) {
    let _ = write!(
        buf,
        "<svg class=\"canvas\" viewBox=\"{}\" preserveAspectRatio=\"xMidYMid meet\" \
         role=\"group\" aria-label=\"Family tree chart\">",
        view_box(model.bounds),
    );
    buf.push_str(
        "<defs><filter id=\"card-shadow\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\">\
         <feDropShadow dx=\"0\" dy=\"1\" stdDeviation=\"1.5\" flood-opacity=\"0.18\"/>\
         </filter></defs>",
    );
    buf.push_str("<g class=\"root\">");
    for link in &model.links {
        write_link(buf, link);
    }
    for node in &model.nodes {
        write_node(buf, node, portraits);
    }
    buf.push_str("</g></svg>");
}

/// A rounded-elbow `<path>` through the link's own anchors; `Partner` links carry
/// the dash via the `partner` class.
fn write_link(buf: &mut String, link: &LayoutLink) {
    let _ = write!(
        buf,
        "<path class=\"link {}\" d=\"{}\" fill=\"none\"/>",
        link_class(link.kind),
        rounded_path(&link.anchors, CORNER),
    );
}

/// The lowercase class suffix for a link kind, matching the canvas's
/// `link.kind.toLowerCase()`.
fn link_class(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::Descent => "descent",
        LinkKind::Partner => "partner",
    }
}

/// The lowercase per-sex edge class, matching the canvas's `sex.toLowerCase()`.
fn sex_class(sex: Sex) -> &'static str {
    match sex {
        Sex::Male => "male",
        Sex::Female => "female",
        Sex::Other => "other",
        Sex::Unknown => "unknown",
    }
}

/// A person card (at its model box) or a union joiner.
fn write_node(buf: &mut String, node: &LayoutNode, portraits: Portraits<'_>) {
    match node.kind {
        NodeKind::Person => write_card(buf, node, portraits),
        NodeKind::Union => {
            let _ = write!(
                buf,
                "<circle class=\"union\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                fmt_coord(node.x + node.width / 2.0),
                fmt_coord(node.y + node.height / 2.0),
                fmt_coord(node.width / 2.0),
            );
        }
    }
}

/// A clipped, shadowed person card translated to its model box — the same shape
/// `TreeNode.svelte` draws (sans the interactive re-root affordances; the export
/// is static). Every user string is escaped.
fn write_card(buf: &mut String, node: &LayoutNode, portraits: Portraits<'_>) {
    // A `Person` always carries content; a content-less node is a `Union`.
    let Some(content) = node.content.as_ref() else {
        return;
    };
    let id = node.id.get();
    let focal = if node.focal { " focal" } else { "" };
    let title = match &content.lifespan {
        Some(ls) => format!("{} ({ls})", content.display_name),
        None => content.display_name.clone(),
    };
    // A portrait is drawn only when enabled AND the node's portrait id resolves
    // (the caller has already redacted living persons, so their id is absent).
    let portrait_url = portraits
        .zip(content.portrait)
        .and_then(|(map, mid)| map.get(&mid));
    // Text slides right of the avatar when a portrait is present; otherwise the
    // historical 14px inset (the card box itself never changes).
    let text_x = if portrait_url.is_some() {
        PORTRAIT_INSET + PORTRAIT_D + PORTRAIT_INSET
    } else {
        14.0
    };
    let _ = write!(
        buf,
        "<g class=\"card{focal}\" transform=\"translate({} {})\">\
         <title>{}</title>\
         <clipPath id=\"card-clip-{id}\"><rect width=\"{}\" height=\"{}\" rx=\"8\"/></clipPath>\
         <rect class=\"bg\" width=\"{}\" height=\"{}\" rx=\"8\" filter=\"url(#card-shadow)\"/>\
         <rect class=\"sex sex-{}\" width=\"4\" height=\"{}\"/>\
         <g clip-path=\"url(#card-clip-{id})\">",
        fmt_coord(node.x),
        fmt_coord(node.y),
        escape(&title),
        fmt_coord(node.width),
        fmt_coord(node.height),
        fmt_coord(node.width),
        fmt_coord(node.height),
        sex_class(content.sex),
        fmt_coord(node.height),
    );
    if let Some(url) = portrait_url {
        write_portrait(buf, id, node.height, url);
    }
    let _ = write!(
        buf,
        "<text class=\"name\" x=\"{}\" y=\"30\">{}</text>",
        fmt_coord(text_x),
        escape(&content.display_name),
    );
    if let Some(ls) = &content.lifespan {
        let _ = write!(
            buf,
            "<text class=\"lifespan\" x=\"{}\" y=\"50\">{}</text>",
            fmt_coord(text_x),
            escape(ls),
        );
    }
    buf.push_str("</g></g>");
}

/// A circular portrait avatar inset at the card's left edge: a clip-circle, the
/// caller-resolved `data:` URL `<image>` (slice-fitted into the circle), and a
/// hairline ring. Drawn inside the card's clip `<g>`, so it never overflows the
/// box (the avatar overlays the existing 220×72 card, no geometry change).
fn write_portrait(buf: &mut String, id: u32, card_height: f64, data_url: &str) {
    let cy = card_height / 2.0;
    let r = PORTRAIT_D / 2.0;
    let cx = PORTRAIT_INSET + r;
    let _ = write!(
        buf,
        "<clipPath id=\"portrait-clip-{id}\"><circle cx=\"{}\" cy=\"{}\" r=\"{}\"/></clipPath>\
         <image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
         preserveAspectRatio=\"xMidYMid slice\" clip-path=\"url(#portrait-clip-{id})\"/>\
         <circle class=\"portrait\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
        fmt_coord(cx),
        fmt_coord(cy),
        fmt_coord(r),
        escape(data_url),
        fmt_coord(PORTRAIT_INSET),
        fmt_coord(cy - r),
        fmt_coord(PORTRAIT_D),
        fmt_coord(PORTRAIT_D),
        fmt_coord(cx),
        fmt_coord(cy),
        fmt_coord(r),
    );
}

/// A polyline `d` with rounded interior elbows over the model's OWN anchors — the
/// Rust mirror of `viewport.ts::roundedPathFromAnchors`. `radius <= 0` or `<= 2`
/// anchors yields the straight `M…L…` form, so rounding is purely additive. NO
/// re-route — the waypoints are the model's; this only shapes the stroke.
fn rounded_path(anchors: &[Point], radius: f64) -> String {
    if anchors.is_empty() {
        return String::new();
    }
    if anchors.len() <= 2 || radius <= 0.0 {
        return straight_path(anchors);
    }
    let first = anchors[0];
    let mut d = format!("M {} {}", fmt_coord(first.x), fmt_coord(first.y));
    for corner in anchors.windows(3) {
        let (prev, cur, next) = (corner[0], corner[1], corner[2]);
        let a = trim_toward(cur, prev, radius); // back along cur→prev
        let b = trim_toward(cur, next, radius); // along cur→next
        let _ = write!(
            d,
            " L {} {} Q {} {} {} {}",
            fmt_coord(a.x),
            fmt_coord(a.y),
            fmt_coord(cur.x),
            fmt_coord(cur.y),
            fmt_coord(b.x),
            fmt_coord(b.y),
        );
    }
    let last = anchors[anchors.len() - 1];
    let _ = write!(d, " L {} {}", fmt_coord(last.x), fmt_coord(last.y));
    d
}

/// A straight polyline `d` through the anchors (`M head L … L tail`); the
/// `radius == 0` / `≤ 2`-anchor base case `rounded_path` falls back to.
fn straight_path(anchors: &[Point]) -> String {
    let mut iter = anchors.iter();
    let Some(head) = iter.next() else {
        return String::new();
    };
    let mut d = format!("M {} {}", fmt_coord(head.x), fmt_coord(head.y));
    for p in iter {
        let _ = write!(d, " L {} {}", fmt_coord(p.x), fmt_coord(p.y));
    }
    d
}

/// A point `dist` from `from` toward `to`, clamped to the segment's half-length so
/// two short adjacent segments can't overshoot into a self-crossing corner (the
/// Rust mirror of `viewport.ts::trimToward`).
fn trim_toward(from: Point, to: Point, dist: f64) -> Point {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = dx.hypot(dy);
    if len > 0.0 {
        let t = dist.min(len / 2.0) / len;
        Point {
            x: from.x + dx * t,
            y: from.y + dy * t,
        }
    } else {
        from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn fmt_coord_strips_trailing_zeros_and_negative_zero() {
        assert_eq!(fmt_coord(75.0), "75");
        assert_eq!(fmt_coord(-158.0), "-158");
        assert_eq!(fmt_coord(12.5), "12.5");
        assert_eq!(fmt_coord(0.0), "0");
        assert_eq!(fmt_coord(-0.0), "0");
    }

    #[test]
    fn view_box_expands_bounds_by_margin_on_every_side() {
        // The exact viewport.test.ts case: a reflected-up ancestors/hourglass chart.
        let bounds = Rect {
            x: -110.0,
            y: 0.0,
            width: 220.0,
            height: 152.0,
        };
        assert_eq!(view_box(bounds), "-158 -48 316 248");
    }

    #[test]
    fn rounded_path_of_two_anchors_is_the_straight_path() {
        // No interior elbow to round — exactly the straight `M…L…` form.
        let a = [pt(0.0, 72.0), pt(0.0, 136.0)];
        assert_eq!(rounded_path(&a, CORNER), "M 0 72 L 0 136");
    }

    #[test]
    fn rounded_path_with_zero_radius_falls_back_to_straight() {
        let a = [pt(0.0, 0.0), pt(0.0, 50.0), pt(40.0, 50.0)];
        assert_eq!(rounded_path(&a, 0.0), straight_path(&a));
        assert_eq!(rounded_path(&a, 0.0), "M 0 0 L 0 50 L 40 50");
    }

    #[test]
    fn rounded_path_rounds_a_three_anchor_elbow() {
        // Trim 25 back along the vertical, quadratic AT the corner, 25 along the
        // horizontal out — the viewport.test.ts elbow case, byte-for-byte.
        let a = [pt(0.0, 0.0), pt(0.0, 100.0), pt(100.0, 100.0)];
        assert_eq!(
            rounded_path(&a, 25.0),
            "M 0 0 L 0 75 Q 0 100 25 100 L 100 100"
        );
    }

    #[test]
    fn rounded_path_clamps_the_trim_to_half_a_short_segment() {
        // The middle segment is length 10; an 8-radius trim clamps to 5 (half), so
        // adjacent short segments can't overshoot into a self-crossing corner.
        let a = [pt(0.0, 0.0), pt(0.0, 10.0), pt(10.0, 10.0)];
        assert_eq!(rounded_path(&a, 8.0), "M 0 0 L 0 5 Q 0 10 5 10 L 10 10");
    }

    #[test]
    fn rounded_path_of_no_anchors_is_empty() {
        assert_eq!(rounded_path(&[], CORNER), "");
    }

    #[test]
    fn sex_and_link_classes_are_lowercase_variant_names() {
        assert_eq!(sex_class(Sex::Male), "male");
        assert_eq!(sex_class(Sex::Unknown), "unknown");
        assert_eq!(link_class(LinkKind::Partner), "partner");
        assert_eq!(link_class(LinkKind::Descent), "descent");
    }
}
