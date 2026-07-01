// nodeDetail.svelte.ts — the canvas detail-popover's state.
//
// Single-clicking a person node on the Tree/Network canvas opens a compact,
// read-only detail popover. This store holds the inspected target and its loaded
// PersonView. Like the chart/search stores it STORES what the command returns
// (api.personGet → PersonView) — it computes nothing. A monotonic seq token
// drops out-of-order responses (rapid clicks across nodes); a load failure
// toasts and closes. No new IPC surface — personGet already backs the Library
// detail pane (D4).

import * as api from "../api";
import { asCommandError } from "../errors";
import type { PersonView } from "../types";
import { toast } from "./toast.svelte";

class NodeDetailStore {
  /** The inspected node (person only for v1), or null when the popover is closed. */
  target = $state<{ kind: "person"; id: number } | null>(null);
  view = $state<PersonView | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  #seq = 0; // newest-open-wins token (guards rapid re-target across nodes)

  /**
   * Inspect person `id`: set the target synchronously so the popover anchors
   * immediately (a "Loading…" shell), then load its PersonView. A newer
   * `open`/`close` drops this result (the seq token); a failure toasts + closes.
   */
  async open(id: number): Promise<void> {
    const seq = ++this.#seq;
    this.target = { kind: "person", id };
    this.view = null; // show the loading shell for the new target — never stale data
    this.loading = true;
    this.error = null;
    try {
      const view = await api.personGet(id);
      if (seq !== this.#seq) return; // a newer open/close superseded this one
      this.view = view;
    } catch (e) {
      if (seq !== this.#seq) return;
      const err = asCommandError(e);
      this.error = err.message;
      toast.pushError(err);
      this.#reset(); // a failed inspect leaves no popover
    } finally {
      if (seq === this.#seq) this.loading = false;
    }
  }

  /** Close the popover and invalidate any in-flight open. */
  close(): void {
    this.#seq++;
    this.#reset();
  }

  #reset(): void {
    this.target = null;
    this.view = null;
    this.error = null;
    this.loading = false;
  }
}

export const nodeDetail = new NodeDetailStore();
