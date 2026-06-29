// search.svelte.ts — the jump-to-person command palette.
//
// A global find-and-jump overlay that searches the WHOLE tree (server-side,
// ranked, debounced) and, on choosing a hit, selects that person in the Library
// and re-roots the Tree ONLY when the Tree view is the active one (else a hidden
// chart would silently re-fetch). Like the chart/library stores it STORES the
// SearchHit[] the command returns — it never ranks or filters client-side (the
// "no logic in the frontend" rule applies to search too). A monotonic seq token
// drops out-of-order responses; the query is debounced like the Library.

import * as api from "../api";
import { asCommandError } from "../errors";
import type { SearchHit } from "../types";
import { chart } from "./chart.svelte";
import { selection } from "./selection.svelte";
import { toast } from "./toast.svelte";
import { ui } from "./ui.svelte";

const SEARCH_DEBOUNCE_MS = 150; // matches library.setQuery / chart slider
const SEARCH_LIMIT = 20; // a palette shows a short ranked list

class SearchPaletteStore {
  isOpen = $state(false);
  query = $state("");
  hits = $state<SearchHit[]>([]);
  /** The keyboard-highlighted result (↑/↓), clamped to `hits`. */
  selectedIndex = $state(0);

  #seq = 0; // newest-search-wins token
  #timer: ReturnType<typeof setTimeout> | undefined;

  /** Open the palette (empty, ready for input). */
  open(): void {
    this.isOpen = true;
  }

  /** Close and reset; invalidates any in-flight search. */
  close(): void {
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#seq++;
    this.isOpen = false;
    this.query = "";
    this.hits = [];
    this.selectedIndex = 0;
  }

  /** Update the query and debounce a re-search. */
  setQuery(query: string): void {
    this.query = query;
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.#run(), SEARCH_DEBOUNCE_MS);
  }

  /** Run the search now: empty query → no hits; else the server-side ranked search. */
  async #run(): Promise<void> {
    const seq = ++this.#seq; // invalidate any older in-flight search
    const q = this.query.trim();
    if (q === "") {
      this.hits = [];
      this.selectedIndex = 0;
      return;
    }
    try {
      const hits = await api.search(q, SEARCH_LIMIT);
      if (seq !== this.#seq) return; // a newer search superseded this one
      this.hits = hits;
      this.selectedIndex = 0;
    } catch (e) {
      if (seq !== this.#seq) return;
      toast.pushError(asCommandError(e));
    }
  }

  /** Move the highlight by `delta`, wrapping within the result list. */
  move(delta: number): void {
    const n = this.hits.length;
    if (n === 0) return;
    this.selectedIndex = (this.selectedIndex + delta + n) % n;
  }

  /** Jump to a hit: select it in the Library, re-root the Tree iff it's open. */
  choose(hit: SearchHit): void {
    const id = hit.individual.id;
    void selection.selectPerson(id);
    if (ui.view === "tree") {
      chart.reroot(id); // re-root only the already-active Tree (no hidden re-fetch)
    } else {
      ui.showLibrary(); // make the selection visible (no-op if already on Library)
    }
    this.close();
  }

  /** Jump to the currently-highlighted hit (Enter). */
  chooseSelected(): void {
    const hit = this.hits[this.selectedIndex];
    if (hit !== undefined) this.choose(hit);
  }
}

export const searchPalette = new SearchPaletteStore();
