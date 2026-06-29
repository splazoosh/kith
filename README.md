# Kith

**A local-first family-tree desktop app for Windows.** Build your family tree
offline — no account, no server, no telemetry. Your data lives in a plain SQLite
file you own.

## Features

- **Editor** — people, families, events, and alternate names, with fuzzy
  genealogical dates (`ABT 1850`, `12 Mar 1887`, ranges).
- **Four chart modes** — ancestors, descendants, hourglass, and **network** (the
  whole connected family graph). Pan/zoom, fit, depth slider, click-to-re-root.
- **Media & portraits**, **sources & citations**, ranked **full-text search**
  with jump-to-person, and in-session **undo**.
- **HTML export** — a single self-contained file; living people redacted by
  default; portraits optional.
- **GEDCOM 5.5.1** import/export, including media, sources, and citations.

## Install

Download the latest Windows installer and run one of:

- **`Kith_1.0.0_x64-setup.exe`** (NSIS) — the recommended **per-user** install
  (no administrator prompt).
- **`Kith_1.0.0_x64_en-US.msi`** (WiX) — a **per-machine** install for managed
  environments.

WebView2 (the system web runtime) is preinstalled on Windows 11; on a machine
without it, the installer downloads it.

## Build from source

Prerequisites: Rust (stable, edition 2024), the Tauri CLI v2, and Node + pnpm.

```bash
# build the whole workspace (core, CLI, Tauri backend)
cargo build --release

# run the desktop app in dev (from the Tauri crate)
pnpm --dir app install
cd crates/kith-tauri && cargo tauri dev

# build the Windows installer (NSIS .exe + WiX .msi under target/release/bundle/)
cd crates/kith-tauri && cargo tauri build

# the CLI
cargo run -p kith-cli -- --help
```

## License

[MIT](./LICENSE) © Kith contributors.
