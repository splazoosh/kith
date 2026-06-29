// library.svelte.ts — the people + families lists and the search query.
//
// People search is server-side via the `search` command, debounced (~150ms);
// an empty query falls back to the full `person_list`. Families filter
// client-side over `family_list` (no family endpoint), reusing the pure
// `filterFamilies` helper. `peopleById` is the once-loaded map that resolves
// both the people list and the family-row labels.

import * as api from "../api";
import { asCommandError } from "../errors";
import { displayName, filterFamilies } from "../format";
import type { Family, Individual } from "../types";
import { toast } from "./toast.svelte";

const SEARCH_DEBOUNCE_MS = 150;

class LibraryStore {
  /** The full, unfiltered person list — feeds `peopleById` and the empty-query view. */
  allPeople = $state<Individual[]>([]);
  /** The full family list (filtered client-side for display). */
  allFamilies = $state<Family[]>([]);
  /** The people currently shown (server search result, or all when query empty). */
  people = $state<Individual[]>([]);
  query = $state("");

  /** id → individual, for family-label resolution and quick lookups. */
  peopleById = $derived(
    new Map(this.allPeople.map((p): [number, Individual] => [p.id, p])),
  );
  /** Families filtered by the query against their resolved label. */
  families = $derived(
    filterFamilies(this.allFamilies, this.peopleById, this.query),
  );

  /** The shown people in stable display-name order. A memoized `$derived`, so the
   *  sort runs **once per `people` change** (a reload or a search result), not on
   *  every render — keeping the sort off the per-render path. The Library
   *  renders this directly. (Full list windowing for several-thousand rows is
   *  a possible later optimization — measure, then window if it
   *  janks at the realistic ceiling.) */
  sortedPeople = $derived(
    [...this.people].sort((a, b) =>
      displayName(a).localeCompare(displayName(b)),
    ),
  );

  #timer: ReturnType<typeof setTimeout> | undefined;

  /** Reload both lists from the open database, then apply the current query. */
  async reload(): Promise<void> {
    try {
      const [people, families] = await Promise.all([
        api.personList(),
        api.familyList(),
      ]);
      this.allPeople = people;
      this.allFamilies = families;
    } catch (e) {
      toast.pushError(asCommandError(e));
      return;
    }
    await this.runSearch();
  }

  /** Update the query and debounce a re-search. */
  setQuery(query: string): void {
    this.query = query;
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.#timer = setTimeout(() => {
      void this.runSearch();
    }, SEARCH_DEBOUNCE_MS);
  }

  /** Run the people search now: empty query → the full list; else the server-side
   *  ranked, multi-field search (the broadening is server-side — `runSearch` just
   *  maps each hit to its person; the Library list is unchanged). */
  async runSearch(): Promise<void> {
    const q = this.query.trim();
    if (q === "") {
      this.people = this.allPeople;
      return;
    }
    try {
      const hits = await api.search(q);
      this.people = hits.map((h) => h.individual);
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  /** Reset to the empty state (on database close). */
  clear(): void {
    if (this.#timer !== undefined) clearTimeout(this.#timer);
    this.allPeople = [];
    this.allFamilies = [];
    this.people = [];
    this.query = "";
  }
}

export const library = new LibraryStore();
