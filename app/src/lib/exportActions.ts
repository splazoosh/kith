// exportActions.ts — the export dialog→save→IPC→toast orchestration,
// mirroring dbActions.ts. The dialog plugin only ever returns a path STRING; the
// Rust `export_html` command does the filesystem IO (least privilege — no `fs`
// ACL added). The component just collects options and calls exportChart.

import { save as saveDialog } from "@tauri-apps/plugin-dialog";

import * as api from "./api";
import { asCommandError } from "./errors";
import { toast } from "./stores/toast.svelte";
import type { ChartMode, Theme } from "./types";

export interface ExportChartOptions {
  root: number;
  mode: ChartMode;
  generations: number;
  theme: Theme;
  includeLiving: boolean;
  /** Embed each person's primary portrait (base64) in the export. */
  includePortraits: boolean;
  /** Used only to seed the save dialog's default filename. */
  defaultName: string;
}

/** Pick a destination via the native save dialog, then write the chart there.
 *  A cancelled save dialog is a silent no-op (the `pickAndCreate` precedent). */
export async function exportChart(opts: ExportChartOptions): Promise<void> {
  // Lightly sanitise the suggested filename — a focal name like "A/B" must not
  // smuggle a path separator into the default; the user can still rename freely.
  const safeName = opts.defaultName.replace(/[/\\]/g, "-");
  const path = await saveDialog({
    defaultPath: `${safeName}.html`,
    filters: [{ name: "HTML", extensions: ["html"] }],
  });
  if (!path) return; // cancelled
  try {
    await api.exportHtml(
      opts.root,
      opts.mode,
      opts.generations,
      opts.theme,
      opts.includeLiving,
      opts.includePortraits,
      path,
    );
    toast.pushNotice(`Exported chart to ${path}`);
  } catch (e) {
    toast.pushError(asCommandError(e));
  }
}

/** Pick a destination via the native save dialog, then write the whole database to a
 *  GEDCOM 5.5.1 file. Whole-tree, un-redacted — there are no options to
 *  gather, so no dialog precedes the save. A cancelled save dialog is a silent no-op. */
export async function exportGedcom(defaultName = "tree"): Promise<void> {
  const safeName = defaultName.replace(/[/\\]/g, "-");
  const path = await saveDialog({
    defaultPath: `${safeName}.ged`,
    filters: [{ name: "GEDCOM", extensions: ["ged"] }],
  });
  if (!path) return; // cancelled
  try {
    await api.exportGedcom(path);
    toast.pushNotice(`Exported GEDCOM to ${path}`);
  } catch (e) {
    toast.pushError(asCommandError(e));
  }
}
