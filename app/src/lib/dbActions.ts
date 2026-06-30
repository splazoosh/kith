// dbActions.ts — the native open/create flows, shared by the DatabaseBar and the
// no-database EmptyState. The dialog plugin only ever returns a path STRING; all
// filesystem IO stays in the Rust commands (least privilege). The `.db`
// filter is a hint — `Store::open` accepts any path.

import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import * as api from "./api";
import { asCommandError } from "./errors";
import { db } from "./stores/db.svelte";
import { importSummary } from "./stores/importSummary.svelte";
import { toast } from "./stores/toast.svelte";
import type { DbInfo, ImportSummary } from "./types";

const FILTERS = [
  { name: "Kith database", extensions: ["db", "sqlite", "kith"] },
];

const GEDCOM_FILTERS = [{ name: "GEDCOM", extensions: ["ged", "gedcom"] }];

const LB_FILTERS = [{ name: "LB export (JSON)", extensions: ["json"] }];

/** Suggest a database filename from a source file's basename (`tree.ged` → `tree.db`). */
function defaultDbName(sourcePath: string): string {
  const base = sourcePath.split(/[/\\]/).pop() ?? "tree";
  const stem = base.replace(/\.[^.]+$/, "") || "tree";
  return `${stem}.db`;
}

/** The shape of a fresh-tree import command (GEDCOM or LB): a new DB + the summary. */
type NewTreeImport = (
  filePath: string,
  dbPath: string,
) => Promise<{ db: DbInfo; summary: ImportSummary }>;

/** The shared back half of a new-tree import: pick a destination `.db` (seeded from
 *  `filePath`'s basename), run the backend `importer`, adopt the freshly opened tree,
 *  and surface its summary through the persistent dialog. Returns the summary, `null`
 *  on a cancelled destination picker, or `null` after toasting a failure (which leaves
 *  the open database untouched — the backend attaches only on success). */
async function importIntoNewTree(
  filePath: string,
  importer: NewTreeImport,
): Promise<ImportSummary | null> {
  const dbPath = await saveDialog({
    defaultPath: defaultDbName(filePath),
    filters: FILTERS,
  });
  if (!dbPath) return null; // cancelled the destination picker

  try {
    const { db: info, summary } = await importer(filePath, dbPath);
    await db.adopt(info); // the backend created + opened the new tree
    importSummary.show(summary); // surface it through the persistent dialog
    return summary;
  } catch (e) {
    toast.pushError(asCommandError(e));
    return null;
  }
}

/** Pick an existing database via the native open dialog, then open it. */
export async function pickAndOpen(): Promise<void> {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: FILTERS,
  });
  if (typeof path === "string") await db.open(path);
}

/** Choose a destination via the native save dialog, then create the database. */
export async function pickAndCreate(): Promise<void> {
  const path = await saveDialog({ defaultPath: "kith.db", filters: FILTERS });
  if (path) await db.create(path);
}

/** Import a GEDCOM into a NEW tree, reachable with or without a database open. Picks
 *  the `.ged` to read, then a destination for a fresh `.db`; the Rust command creates
 *  the database, imports (a new tree — never appending into an open one), and opens it.
 *  On success the new tree is adopted (Library refreshed) and the summary is surfaced
 *  for the persistent dialog. Either cancelled dialog is a silent no-op; a failure
 *  pushes an error toast and leaves the open database untouched. Returns the summary
 *  (or `null` on cancel/failure) for tests/callers. */
export async function importGedcom(): Promise<ImportSummary | null> {
  const filePath = await openDialog({
    multiple: false,
    directory: false,
    filters: GEDCOM_FILTERS,
  });
  if (typeof filePath !== "string") return null; // cancelled the file picker
  return importIntoNewTree(filePath, api.importGedcom);
}

/** Import an "LB" JSON export into a NEW tree — the same fresh-tree flow as
 *  {@link importGedcom} (pick the `.json`, then a destination `.db`; the Rust command
 *  creates + imports + opens it). Reachable with or without a database open; a
 *  cancelled dialog is a silent no-op and a failure toasts and leaves the open
 *  database untouched. Returns the summary (or `null` on cancel/failure). */
export async function importLb(): Promise<ImportSummary | null> {
  const filePath = await openDialog({
    multiple: false,
    directory: false,
    filters: LB_FILTERS,
  });
  if (typeof filePath !== "string") return null; // cancelled the file picker
  return importIntoNewTree(filePath, api.importLb);
}
