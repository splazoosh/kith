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
import type { ImportSummary } from "./types";

const FILTERS = [
  { name: "Kith database", extensions: ["db", "sqlite", "kith"] },
];

const GEDCOM_FILTERS = [{ name: "GEDCOM", extensions: ["ged", "gedcom"] }];

/** Suggest a database filename from the chosen GEDCOM's basename (`tree.ged` → `tree.db`). */
function defaultDbName(gedPath: string): string {
  const base = gedPath.split(/[/\\]/).pop() ?? "tree";
  const stem = base.replace(/\.[^.]+$/, "") || "tree";
  return `${stem}.db`;
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

  const dbPath = await saveDialog({
    defaultPath: defaultDbName(filePath),
    filters: FILTERS,
  });
  if (!dbPath) return null; // cancelled the destination picker

  try {
    const { db: info, summary } = await api.importGedcom(filePath, dbPath);
    await db.adopt(info); // the backend created + opened the new tree
    importSummary.show(summary); // surface it through the persistent dialog
    return summary;
  } catch (e) {
    toast.pushError(asCommandError(e));
    return null;
  }
}
