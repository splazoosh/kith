//! The light + dark design-token values, mirrored from `app/src/styles/tokens.css`.
//! The export `<style>` is the file's source of truth, so the values
//! must exist in Rust. Hand-maintained in sync with `tokens.css`; drift is
//! snapshot-guarded (tests/render_html.rs). **If you change a value here, change it
//! in `tokens.css` too — they are one palette in two places.**

/// One theme's resolved token values (only the tokens the chart marks read).
pub(crate) struct Palette {
    /// `--color-paper` — page + canvas background.
    pub paper: &'static str,
    /// `--color-surface` — card fill.
    pub surface: &'static str,
    /// `--color-hairline` — card stroke.
    pub hairline: &'static str,
    /// `--color-ink` — name text.
    pub ink: &'static str,
    /// `--color-ink-soft` — lifespan text + union fill.
    pub ink_soft: &'static str,
    /// `--tree-link` — connector stroke.
    pub tree_link: &'static str,
    /// `--tree-focal` — focal ring stroke.
    pub tree_focal: &'static str,
    /// `--tree-sex-male`.
    pub sex_male: &'static str,
    /// `--tree-sex-female`.
    pub sex_female: &'static str,
    /// `--tree-sex-other` / `--tree-sex-unknown` (one warm neutral for both).
    pub sex_unknown: &'static str,
}

/// Font stacks are theme-independent (system fonts — never a web-font fetch).
pub(crate) const FONT_SERIF: &str =
    "\"Iowan Old Style\", \"Palatino Linotype\", Palatino, Georgia, serif";
/// The sans stack for metadata + UI (system fonts only).
pub(crate) const FONT_SANS: &str =
    "system-ui, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif";

/// The light palette (`:root` in `tokens.css`).
pub(crate) const LIGHT: Palette = Palette {
    paper: "#faf8f4",
    surface: "#ffffff",
    hairline: "#e2dcd2",
    ink: "#22201c",
    ink_soft: "#5a554c",
    tree_link: "#8c8576",
    tree_focal: "#6b5d44",
    sex_male: "#5b6b80",
    sex_female: "#9a5c5c",
    sex_unknown: "#8a8275",
};

/// The dark palette (a true theme, not an inversion — the lifted `--tree-*` and
/// surface values from `tokens.css`'s dark block).
pub(crate) const DARK: Palette = Palette {
    paper: "#1a1815",
    surface: "#221f1b",
    hairline: "#36322b",
    ink: "#ece6da",
    ink_soft: "#b7ae9d",
    tree_link: "#6f6a5e",
    tree_focal: "#c9b493",
    sex_male: "#7e93ad",
    sex_female: "#bd8585",
    sex_unknown: "#8a8275",
};
