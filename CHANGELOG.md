# Changelog

All notable changes to Kith are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0-rc.1] - 2026-06-29

First public preview — the release candidate for Kith 1.0.0. A complete,
local-first family-tree desktop app for Windows: a Rust core, a Tauri 2 shell,
and SQLite storage. No server, no account, no telemetry — your data lives in a
plain SQLite file you own.

### Added

- **Editor** — people, families, events, and alternate names, with fuzzy
  genealogical dates (`ABT 1850`, `12 Mar 1887`, ranges) previewed as you type.
- **Charts** — four navigable SVG modes: ancestors, descendants, hourglass, and
  network (the whole connected family graph). Pan/zoom, fit-to-screen, a
  generation-depth slider, and click-to-re-root. Light/dark themes,
  reduced-motion- and AA-contrast-aware, keyboard-navigable.
- **Media & portraits** — attach photos; the primary portrait flows onto the
  live canvas and into the HTML export.
- **Sources & citations** — attach provenance to events in the GUI (and to
  people/families via the CLI and GEDCOM).
- **Search** — ranked, multi-field full-text search with a jump-to-person
  palette.
- **Undo** — in-session, multi-level undo of destructive edits.
- **HTML export** — a single self-contained file; living people redacted by
  default; portraits optional.
- **GEDCOM 5.5.1** — deterministic whole-tree import/export, including media,
  sources, and citations.
- **CLI** — a scriptable `kith` binary mirroring the core (`init`, `person`,
  `family`, `event`, `query`, `search`, `media`, `source`, `citation`, `export`,
  `import`, `db`).
- **Keyboard shortcuts** — jump-to-person, undo, new person, view switches, and
  a `?` help overlay.
- **Windows installers** — a per-user NSIS `.exe` and a per-machine WiX `.msi`.

### Notes

- This preview ships **unsigned**. Windows SmartScreen will warn about an
  "unknown publisher" — choose **More info → Run anyway** to proceed. Code
  signing is wired into the release pipeline and activates once a signing
  certificate is configured.
- WebView2 (the system web runtime) is preinstalled on Windows 11; on a machine
  without it, the installer downloads it.

[1.0.0-rc.1]: https://github.com/splazoosh/kith/releases/tag/v1.0.0-rc.1
