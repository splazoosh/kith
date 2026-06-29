//! The `html` entry — composes redaction → the SVG body into a
//! complete, self-contained `<!doctype html>` document: inline token `<style>` for
//! both themes + a print block, a minimal toolbar (theme toggle + fit), and a
//! dependency-free pan/zoom + toggle script. Infallible; deterministic.

use std::fmt::Write as _;

use super::escape::escape;
use super::redact::redact_living;
use super::svg::write_svg;
use super::theme::{DARK, FONT_SANS, FONT_SERIF, LIGHT, Palette};
use super::{HtmlExportOptions, Theme};
use crate::layout::{ChartMode, LayoutModel};

/// Render `model` to a complete, self-contained HTML document per `options`.
///
/// Reads only the model's positions/anchors/bounds (no geometry is recomputed),
/// applies living-person redaction unless `options.include_living`, and emits
/// inline `<svg>` + inline `<style>` (both themes + print) + a dependency-free
/// pan/zoom & theme-toggle script. **Deterministic** — same `(model, options)`
/// ⇒ byte-identical output (no timestamp/random). Self-contained — no external
/// `href`/`src`/`@import`/web-font.
///
/// # Examples
/// ```
/// # use kith_core::prelude::*;
/// # use kith_core::render::{html, HtmlExportOptions};
/// # fn main() -> Result<()> {
/// let store = Store::open_in_memory()?;
/// let root = store
///     .create_individual(&NewIndividual { given_name: Some("Ada".into()), ..Default::default() })?
///     .id;
/// let model = compute_layout(&store, root, ChartMode::Descendants, 4)?;
/// let doc = html(&model, &HtmlExportOptions::default());
/// assert!(doc.starts_with("<!doctype html>"));
/// assert!(doc.contains("<svg"));
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn html(model: &LayoutModel, options: &HtmlExportOptions) -> String {
    // `Cow` — borrows when `include_living`, owns a redacted clone otherwise.
    let model = redact_living(model, options.include_living);
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| derive_title(&model));
    let theme_attr = match options.theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    };

    let mut buf = String::with_capacity(8192);
    let _ = write!(
        buf,
        "<!doctype html>\n<html lang=\"en\" data-theme=\"{theme_attr}\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n",
        escape(&title),
    );
    write_style(&mut buf);
    buf.push_str("</head>\n<body>\n");
    buf.push_str(
        "<div class=\"toolbar\">\
         <button type=\"button\" id=\"theme-toggle\">Theme</button>\
         <button type=\"button\" id=\"fit\">Fit</button>\
         </div>\n",
    );
    // Portraits are drawn only when enabled; the map was resolved by the caller
    // over the redacted model, so a living person's id is already gone.
    let portraits = options.portraits.then_some(&options.portrait_urls);
    write_svg(&mut buf, &model, portraits);
    buf.push('\n');
    write_script(&mut buf);
    buf.push_str("\n</body>\n</html>\n");
    buf
}

/// A deterministic document title from the (possibly redacted) focal node + mode,
/// e.g. `"Ada Lovelace — Descendants"` (or `"Living — Descendants"` when redacted).
/// No `now()` — determinism is what makes the snapshots byte-stable.
fn derive_title(model: &LayoutModel) -> String {
    let mode = match model.mode {
        ChartMode::Ancestors => "Ancestors",
        ChartMode::Descendants => "Descendants",
        ChartMode::Hourglass => "Hourglass",
        ChartMode::Network => "Network",
    };
    match model
        .nodes
        .iter()
        .find(|n| n.focal)
        .and_then(|n| n.content.as_ref())
    {
        Some(content) => format!("{} \u{2014} {mode}", content.display_name),
        None => format!("Family tree \u{2014} {mode}"),
    }
}

/// The inline `<style>`: the light tokens under `:root`, the dark tokens under both
/// the system-pref media query and `[data-theme="dark"]` (the `tokens.css` two-layer
/// scheme), the canvas/card/link rules, a `@media print` block, and a reduced-motion
/// neutraliser.
fn write_style(buf: &mut String) {
    buf.push_str("<style>\n:root {\n");
    write_palette_vars(buf, &LIGHT);
    let _ = write!(
        buf,
        "  --font-serif: {FONT_SERIF};\n  --font-sans: {FONT_SANS};\n",
    );
    buf.push_str(
        "}\n@media (prefers-color-scheme: dark) {\n  :root:not([data-theme=\"light\"]) {\n",
    );
    write_palette_vars(buf, &DARK);
    buf.push_str("  }\n}\n:root[data-theme=\"dark\"] {\n");
    write_palette_vars(buf, &DARK);
    buf.push_str("}\n");
    buf.push_str(STATIC_CSS);
    buf.push_str("</style>\n");
}

/// Emit one palette as `--token: value;` custom-property declarations.
fn write_palette_vars(buf: &mut String, p: &Palette) {
    let _ = write!(
        buf,
        "  --color-paper: {};\n  --color-surface: {};\n  --color-hairline: {};\n  \
         --color-ink: {};\n  --color-ink-soft: {};\n  --tree-link: {};\n  \
         --tree-focal: {};\n  --tree-sex-male: {};\n  --tree-sex-female: {};\n  \
         --tree-sex-other: {};\n  --tree-sex-unknown: {};\n",
        p.paper,
        p.surface,
        p.hairline,
        p.ink,
        p.ink_soft,
        p.tree_link,
        p.tree_focal,
        p.sex_male,
        p.sex_female,
        p.sex_unknown,
        p.sex_unknown,
    );
}

/// Wrap the dependency-free pan/zoom + theme-toggle script in an inline `<script>`.
fn write_script(buf: &mut String) {
    buf.push_str("<script>\n");
    buf.push_str(PANZOOM_JS);
    buf.push_str("\n</script>");
}

/// The static (theme-independent) CSS: the canvas/card/link rules ported from
/// `TreeCanvas`/`TreeNode`, a toolbar style, the print block (reset the pan/zoom
/// transform so the whole tree prints), and a reduced-motion neutraliser.
const STATIC_CSS: &str = r#"html, body { margin: 0; height: 100%; background: var(--color-paper); }
.toolbar { position: fixed; top: 0.75rem; right: 0.75rem; display: flex; gap: 0.5rem; font-family: var(--font-sans); }
.toolbar button { font: inherit; padding: 0.35rem 0.7rem; color: var(--color-ink); background: var(--color-surface); border: 1px solid var(--color-hairline); border-radius: 8px; cursor: pointer; }
.canvas { display: block; width: 100%; height: 100%; background: var(--color-paper); touch-action: none; }
.card .bg { fill: var(--color-surface); stroke: var(--color-hairline); stroke-width: 1; }
.card.focal .bg { stroke: var(--tree-focal); stroke-width: 2.5; }
.name { font-family: var(--font-serif); font-size: 1rem; fill: var(--color-ink); }
.lifespan { font-family: var(--font-sans); font-size: 0.8125rem; fill: var(--color-ink-soft); }
.portrait { fill: none; stroke: var(--color-hairline); stroke-width: 1; }
.sex-male { fill: var(--tree-sex-male); }
.sex-female { fill: var(--tree-sex-female); }
.sex-other, .sex-unknown { fill: var(--tree-sex-unknown); }
.union { fill: var(--color-ink-soft); opacity: 0.6; }
.link { stroke: var(--tree-link); stroke-width: 1.5; fill: none; }
.link.partner { stroke-dasharray: 5 4; }
@media print {
  .toolbar { display: none; }
  .root { transform: none !important; }
  .canvas { width: 100%; height: auto; }
}
@media (prefers-reduced-motion: reduce) { * { transition: none !important; } }
"#;

/// The dependency-free pan/zoom + theme-toggle script. Writes one `transform`
/// onto the root `<g>` (the same single-transform model as the canvas); wheel zooms
/// about the pointer, pointer-drag pans, "Fit" resets to identity, "Theme" flips
/// `[data-theme]`. Progressive enhancement: with the script removed the SVG still
/// renders, framed by its `viewBox` and legible.
const PANZOOM_JS: &str = r#"(function () {
  var svg = document.querySelector('.canvas');
  var g = document.querySelector('.root');
  if (!svg || !g) return;
  var t = { x: 0, y: 0, k: 1 };
  function apply() {
    g.setAttribute('transform', 'translate(' + t.x + ' ' + t.y + ') scale(' + t.k + ')');
  }
  function toLocal(ev) {
    var ctm = svg.getScreenCTM();
    if (!ctm) return { x: ev.clientX, y: ev.clientY };
    var inv = ctm.inverse();
    return {
      x: ev.clientX * inv.a + ev.clientY * inv.c + inv.e,
      y: ev.clientX * inv.b + ev.clientY * inv.d + inv.f
    };
  }
  svg.addEventListener('wheel', function (ev) {
    ev.preventDefault();
    var p = toLocal(ev);
    var mx = (p.x - t.x) / t.k;
    var my = (p.y - t.y) / t.k;
    var factor = ev.deltaY < 0 ? 1.1 : 1 / 1.1;
    t.k = Math.max(0.1, Math.min(8, t.k * factor));
    t.x = p.x - mx * t.k;
    t.y = p.y - my * t.k;
    apply();
  }, { passive: false });
  var dragging = false, lastX = 0, lastY = 0;
  svg.addEventListener('pointerdown', function (ev) {
    dragging = true;
    lastX = ev.clientX;
    lastY = ev.clientY;
    if (svg.setPointerCapture) svg.setPointerCapture(ev.pointerId);
  });
  svg.addEventListener('pointermove', function (ev) {
    if (!dragging) return;
    var ctm = svg.getScreenCTM();
    var sx = ctm ? ctm.a : 1;
    var sy = ctm ? ctm.d : 1;
    t.x += (ev.clientX - lastX) / sx;
    t.y += (ev.clientY - lastY) / sy;
    lastX = ev.clientX;
    lastY = ev.clientY;
    apply();
  });
  function endDrag() { dragging = false; }
  svg.addEventListener('pointerup', endDrag);
  svg.addEventListener('pointercancel', endDrag);
  var fitBtn = document.getElementById('fit');
  if (fitBtn) fitBtn.addEventListener('click', function () {
    t = { x: 0, y: 0, k: 1 };
    apply();
  });
  var themeBtn = document.getElementById('theme-toggle');
  if (themeBtn) themeBtn.addEventListener('click', function () {
    var root = document.documentElement;
    root.setAttribute('data-theme', root.getAttribute('data-theme') === 'dark' ? 'light' : 'dark');
  });
})();"#;
