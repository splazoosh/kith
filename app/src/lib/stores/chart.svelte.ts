// chart.svelte.ts — the chart's view params + the fetched model.
// The reactive driver: rootId/mode/
// generations are $state; every control mutates them and calls the ONE private
// #load() → api.computeLayout (re-fetch, never recompute). A monotonic seq
// token drops out-of-order responses; the slider is debounced. It STORES a
// model — it never builds one (the cardinal rule).
import * as api from "../api";
import { asCommandError } from "../errors";
import type { ChartMode, LayoutModel } from "../types";
import { toast } from "./toast.svelte";

export const DEFAULT_MODE: ChartMode = "Descendants";
export const DEFAULT_GENERATIONS = 4;
const SLIDER_DEBOUNCE_MS = 150; // matches library.setQuery / PersonPicker

class ChartStore {
  rootId = $state<number | null>(null);
  mode = $state<ChartMode>(DEFAULT_MODE);
  generations = $state(DEFAULT_GENERATIONS);

  model = $state<LayoutModel | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  #seq = 0; // newest-load-wins token (guards rapid re-root / slider)
  #timer: ReturnType<typeof setTimeout> | undefined;

  /** Enter the chart on `rootId`, keeping the current mode + depth, and load. */
  view(rootId: number): void {
    this.rootId = rootId;
    void this.#load();
  }

  /** Re-root on a clicked/Entered person; mode + depth unchanged. */
  reroot(personId: number): void {
    if (personId === this.rootId) return;
    this.rootId = personId;
    void this.#load();
  }

  /** Switch chart mode (any of the four). Network ignores depth — it lays out the
   * whole connected component — but we still pass the current value; the backend
   * disregards it (the depth control is disabled in Network; see TreeView). */
  setMode(mode: ChartMode): void {
    if (mode === this.mode) return;
    this.mode = mode;
    void this.#load();
  }

  /** Set generation depth; debounced so a slider drag can't storm the IPC. */
  setGenerations(generations: number): void {
    this.generations = generations;
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.#load(), SLIDER_DEBOUNCE_MS);
  }

  /** Fetch the WHOLE model for the current (root, mode, generations) tuple. */
  async #load(): Promise<void> {
    const root = this.rootId;
    if (root === null) return;
    const seq = ++this.#seq;
    this.loading = true;
    this.error = null;
    try {
      const model = await api.computeLayout(root, this.mode, this.generations);
      if (seq !== this.#seq) return; // a newer load superseded this one
      this.model = model; // NB: not nulled on entry — the prior chart stays under the veil
    } catch (e) {
      if (seq !== this.#seq) return;
      const err = asCommandError(e);
      this.error = err.message;
      this.model = null;
      toast.pushError(err);
    } finally {
      if (seq === this.#seq) this.loading = false;
    }
  }

  clear(): void {
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#seq++; // invalidate any in-flight load
    this.rootId = null;
    this.mode = DEFAULT_MODE;
    this.generations = DEFAULT_GENERATIONS;
    this.model = null;
    this.loading = false;
    this.error = null;
  }
}

export const chart = new ChartStore();
