// db.svelte.ts — the open-database lifecycle over the typed client.
//
// `refresh()` surfaces the restart-reopen (`db_current`); `open`/`create`
// attach a file then reload the library; `close` clears everything. Each
// await-wraps the client and, on a thrown CommandError, pushes a toast and
// leaves `current` unchanged — the UI never white-screens.

import * as api from "../api";
import { asCommandError } from "../errors";
import type { DbInfo } from "../types";
import { chart } from "./chart.svelte";
import { library } from "./library.svelte";
import { selection } from "./selection.svelte";
import { sources } from "./sources.svelte";
import { toast } from "./toast.svelte";
import { ui } from "./ui.svelte";
import { undo } from "./undo.svelte";

class DbStore {
  current = $state<DbInfo | null>(null);

  /** On launch: pick up the database reopened on restart, if any. */
  async refresh(): Promise<void> {
    try {
      this.current = await api.dbCurrent();
    } catch (e) {
      toast.pushError(asCommandError(e));
      return;
    }
    if (this.current) await Promise.all([library.reload(), sources.reload()]);
  }

  async open(path: string): Promise<void> {
    try {
      this.current = await api.dbOpen(path);
    } catch (e) {
      toast.pushError(asCommandError(e));
      return;
    }
    selection.clear();
    chart.clear();
    undo.reset(); // the server cleared its stack; mirror it client-side
    await Promise.all([library.reload(), sources.reload()]);
  }

  async create(path: string): Promise<void> {
    try {
      this.current = await api.dbCreate(path);
    } catch (e) {
      toast.pushError(asCommandError(e));
      return;
    }
    selection.clear();
    chart.clear();
    undo.reset();
    await Promise.all([library.reload(), sources.reload()]);
  }

  /** Adopt a database the backend already opened (e.g. after a fresh GEDCOM import,
   *  which creates + attaches a new tree). Mirrors `open`/`create`'s reset + reload
   *  but takes the `DbInfo` directly — the command did the attach. */
  async adopt(info: DbInfo): Promise<void> {
    this.current = info;
    selection.clear();
    chart.clear();
    undo.reset();
    await Promise.all([library.reload(), sources.reload()]);
  }

  async close(): Promise<void> {
    try {
      await api.dbClose();
    } catch (e) {
      toast.pushError(asCommandError(e));
      return;
    }
    this.current = null;
    library.clear();
    selection.clear();
    chart.clear();
    sources.clear();
    undo.reset();
    ui.showLibrary(); // a closed DB has no Sources/Tree to show
  }
}

export const db = new DbStore();
