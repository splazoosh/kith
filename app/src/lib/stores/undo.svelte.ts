// undo.svelte.ts — the session undo affordance. A thin client over
// the one `undo` command: the SERVER owns the stack and the restore (kith-tauri
// + kith_core); this store only tracks the depth/top-label for the action toast
// and the registry, triggers `api.undo()`, and refreshes the affected views.
//
// `recordDelete(label)` is called by each delete flow after a successful delete:
// it bumps the depth and shows the "Deleted X · Undo" action toast. `runUndo()`
// (the toast button or Ctrl/Cmd+Z) pops + restores server-side and refreshes.

import * as api from "../api";
import { asCommandError } from "../errors";
import { chart } from "./chart.svelte";
import { library } from "./library.svelte";
import { selection } from "./selection.svelte";
import { sources } from "./sources.svelte";
import { toast } from "./toast.svelte";
import { ui } from "./ui.svelte";

class UndoStore {
  /** Undoable deletions remaining this session (mirrors the server stack depth). */
  depth = $state(0);
  /** The most recent deletion's label (for the action toast). */
  topLabel = $state<string | null>(null);

  /** Record a just-completed delete: bump the depth and show the action toast. */
  recordDelete(label: string): void {
    this.depth += 1;
    this.topLabel = label;
    toast.pushAction(`Deleted ${label}`, "Undo", () => {
      void this.runUndo();
    });
  }

  /** Pop + restore the last deletion (the toast button or Ctrl/Cmd+Z), then
   *  refresh the affected views. A `null` outcome means the stack was already
   *  empty; an error means the restore failed (a reused id) and
   *  the server dropped the entry. */
  async runUndo(): Promise<void> {
    if (this.depth === 0) return;
    try {
      const outcome = await api.undo();
      if (outcome === null) {
        this.depth = 0;
        this.topLabel = null;
        return;
      }
      this.depth = outcome.remaining;
      this.topLabel = null;
      toast.pushNotice(`Restored ${outcome.label}`);
      await this.#refresh(outcome.kind);
    } catch (e) {
      this.depth = Math.max(0, this.depth - 1);
      this.topLabel = null;
      toast.pushError(asCommandError(e));
    }
  }

  /** Refresh the views a restore could have changed, keyed lightly on `kind`. */
  async #refresh(kind: string): Promise<void> {
    await library.reload(); // people/families counts (and the detail's existence)
    await selection.reselect(); // the open detail pane's events/names/citations/media
    if (kind === "source" || kind === "citation" || ui.view === "sources") {
      await sources.reload();
    }
    // Re-fetch the chart only when the Tree is the active view (no hidden re-fetch).
    if (ui.view === "tree" && chart.rootId !== null) {
      chart.view(chart.rootId);
    }
  }

  /** Reset the client depth/label — mirrors the server `clear_undo` on a DB change. */
  reset(): void {
    this.depth = 0;
    this.topLabel = null;
  }
}

export const undo = new UndoStore();
