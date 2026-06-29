# Kith frontend

The Kith desktop UI — a **Svelte 5 + TypeScript + Vite** single-page app that
the Tauri shell loads as its WebView. Layout/graph math never lives here; the
frontend renders the `LayoutModel` it receives from `kith-core` and adds
interaction only. Talk to the backend through one thin typed `invoke` client
(arriving in Phase 2.1) — components don't call `invoke` directly.

## Commands

Run from the repo root (pnpm 9):

```bash
pnpm --dir app install     # install deps (uses the committed pnpm-lock.yaml)
pnpm --dir app dev         # Vite dev server on http://localhost:1420
pnpm --dir app build       # emit app/dist (what Tauri's frontendDist points at)
pnpm --dir app check       # svelte-check + tsc
pnpm --dir app test        # vitest (node env smoke test)
```

The desktop window is driven by Tauri, not Vite directly — from
`crates/kith-tauri/` run `cargo tauri dev` (it starts Vite via
`beforeDevCommand` and opens the window with HMR). See `CLAUDE.md` for the
cross-directory `tauri.conf.json` wiring rules.
