// sources.svelte.ts — the source catalogue.
//
// A thin list/cache over `source_list`, reloaded on each write and on database
// open/close (wired in db.svelte.ts, the Library-store grain). It backs the
// Sources management view AND the citations editor's source picker, so a source
// created in one surface is immediately attachable in the other.

import * as api from "../api";
import { asCommandError } from "../errors";
import type { Source } from "../types";
import { toast } from "./toast.svelte";

class SourcesStore {
  /** Every source, ascending id (the `source_list` order). */
  all = $state<Source[]>([]);

  /** Reload the catalogue from the open database. */
  async reload(): Promise<void> {
    try {
      this.all = await api.sourceList();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  /** Reset on database close. */
  clear(): void {
    this.all = [];
  }
}

export const sources = new SourcesStore();
