// search.svelte.test.ts — the jump-to-person palette store. `api`,
// `toast`, `selection`, and `chart` are mocked; `ui` is real (so view-dependent
// re-root is exercised). The store STORES the SearchHit[] the command returns —
// these tests pin the debounce, the out-of-order guard, and the choose/re-root
// behaviour (select in Library always; re-root only when the Tree is active).

import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../api", () => ({ search: vi.fn() }));
vi.mock("./toast.svelte", () => ({ toast: { pushError: vi.fn() } }));
vi.mock("./selection.svelte", () => ({ selection: { selectPerson: vi.fn() } }));
vi.mock("./chart.svelte", () => ({ chart: { reroot: vi.fn() } }));

import * as api from "../api";
import { CommandError } from "../errors";
import type { SearchHit } from "../types";
import { chart } from "./chart.svelte";
import { searchPalette } from "./search.svelte";
import { selection } from "./selection.svelte";
import { toast } from "./toast.svelte";
import { ui } from "./ui.svelte";

const hit = (id: number, context: string | null = null): SearchHit => ({
  individual: {
    id,
    given_name: null,
    surname: null,
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Unknown",
    living: false,
    notes: null,
  },
  context,
});

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.useRealTimers();
  vi.mocked(api.search).mockReset().mockResolvedValue([]);
  vi.mocked(toast.pushError).mockReset();
  vi.mocked(selection.selectPerson).mockReset();
  vi.mocked(chart.reroot).mockReset();
  searchPalette.close();
  ui.showLibrary();
});

test("open shows the palette; close resets query + hits", () => {
  searchPalette.open();
  expect(searchPalette.isOpen).toBe(true);
  searchPalette.hits = [hit(1)];
  searchPalette.close();
  expect(searchPalette.isOpen).toBe(false);
  expect(searchPalette.query).toBe("");
  expect(searchPalette.hits).toEqual([]);
  expect(searchPalette.selectedIndex).toBe(0);
});

test("setQuery debounces a drag into one search and stores the ranked hits", async () => {
  vi.useFakeTimers();
  vi.mocked(api.search).mockResolvedValue([hit(1, "Lovelace"), hit(2)]);
  searchPalette.open();
  searchPalette.setQuery("a");
  searchPalette.setQuery("ad");
  searchPalette.setQuery("ada");
  await vi.advanceTimersByTimeAsync(200);
  expect(api.search).toHaveBeenCalledTimes(1);
  expect(api.search).toHaveBeenCalledWith("ada", 20); // the palette's short-list limit
  expect(searchPalette.hits.map((h) => h.individual.id)).toEqual([1, 2]);
});

test("an empty/whitespace query clears the hits without calling the command", async () => {
  vi.useFakeTimers();
  searchPalette.open();
  searchPalette.hits = [hit(1)];
  searchPalette.setQuery("   ");
  await vi.advanceTimersByTimeAsync(200);
  expect(api.search).not.toHaveBeenCalled();
  expect(searchPalette.hits).toEqual([]);
});

test("an out-of-order resolve does not clobber a newer search", async () => {
  const slow = deferred<SearchHit[]>();
  vi.mocked(api.search)
    .mockReturnValueOnce(slow.promise) // first query — resolves LATER
    .mockResolvedValueOnce([hit(2)]); // second query — resolves now

  searchPalette.open();
  searchPalette.setQuery("a");
  await new Promise((r) => setTimeout(r, 200));
  searchPalette.setQuery("ab");
  await new Promise((r) => setTimeout(r, 200));
  expect(searchPalette.hits.map((h) => h.individual.id)).toEqual([2]);

  slow.resolve([hit(1)]); // the stale search finally resolves…
  await new Promise((r) => setTimeout(r, 0));
  expect(searchPalette.hits.map((h) => h.individual.id)).toEqual([2]); // …and is dropped
});

test("a failed search toasts the error", async () => {
  vi.mocked(api.search).mockRejectedValueOnce(new CommandError("io", "boom"));
  searchPalette.open();
  searchPalette.setQuery("ada");
  await new Promise((r) => setTimeout(r, 200));
  expect(toast.pushError).toHaveBeenCalledOnce();
});

test("move wraps the highlight within the result list", () => {
  searchPalette.hits = [hit(1), hit(2), hit(3)];
  searchPalette.selectedIndex = 0;
  searchPalette.move(-1);
  expect(searchPalette.selectedIndex).toBe(2); // wraps to the last
  searchPalette.move(1);
  expect(searchPalette.selectedIndex).toBe(0); // wraps back to the first
});

test("choose selects in the Library and does NOT re-root when off the Tree", () => {
  ui.showLibrary();
  searchPalette.open();
  searchPalette.choose(hit(7));
  expect(selection.selectPerson).toHaveBeenCalledWith(7);
  expect(chart.reroot).not.toHaveBeenCalled();
  expect(searchPalette.isOpen).toBe(false);
});

test("choose re-roots the Tree when the Tree view is the active one", () => {
  ui.showTree();
  searchPalette.open();
  searchPalette.choose(hit(9));
  expect(selection.selectPerson).toHaveBeenCalledWith(9);
  expect(chart.reroot).toHaveBeenCalledWith(9);
});

test("chooseSelected jumps to the highlighted hit", () => {
  searchPalette.hits = [hit(1), hit(2), hit(3)];
  searchPalette.selectedIndex = 1;
  searchPalette.chooseSelected();
  expect(selection.selectPerson).toHaveBeenCalledWith(2);
});
