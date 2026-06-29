// JumpToPerson.test.ts — jsdom test for the jump-to-person palette.
// `selection`/`chart` are mocked so a jump is observable; the real palette store
// is driven by seeding its $state before render. Asserts the render (hits + the
// why-matched context), click-to-jump (+ close), and keyboard select (↑/Enter).
// The store's debounce/ranking is the store test's job — this asserts the UI.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({ search: vi.fn().mockResolvedValue([]) }));
vi.mock("../lib/stores/selection.svelte", () => ({
  selection: { selectPerson: vi.fn() },
}));
vi.mock("../lib/stores/chart.svelte", () => ({ chart: { reroot: vi.fn() } }));

import { searchPalette } from "../lib/stores/search.svelte";
import { selection } from "../lib/stores/selection.svelte";
import { ui } from "../lib/stores/ui.svelte";
import type { SearchHit } from "../lib/types";
import JumpToPerson from "./JumpToPerson.svelte";

const hit = (
  id: number,
  given: string,
  surname: string,
  context: string | null = null,
): SearchHit => ({
  individual: {
    id,
    given_name: given,
    surname,
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Unknown",
    living: false,
    notes: null,
  },
  context,
});

beforeEach(() => {
  vi.mocked(selection.selectPerson).mockReset();
  searchPalette.close();
  ui.showLibrary();
});

test("renders nothing when the palette is closed", () => {
  const { queryByRole } = render(JumpToPerson);
  expect(queryByRole("dialog")).toBeNull();
});

test("renders the ranked hits with their why-matched context", () => {
  searchPalette.open();
  searchPalette.hits = [hit(1, "Ada", "Lovelace", "née Byron")];
  const { getByText } = render(JumpToPerson);
  expect(getByText("Ada Lovelace")).toBeTruthy();
  expect(getByText("née Byron")).toBeTruthy();
});

test("clicking a result jumps to that person and closes the palette", async () => {
  searchPalette.open();
  searchPalette.hits = [hit(7, "Jane", "Doe")];
  const { getByText, queryByRole } = render(JumpToPerson);
  await fireEvent.click(getByText("Jane Doe"));
  expect(selection.selectPerson).toHaveBeenCalledWith(7);
  expect(queryByRole("dialog")).toBeNull();
});

test("ArrowDown then Enter jumps to the highlighted hit", async () => {
  searchPalette.open();
  searchPalette.hits = [hit(1, "Amy", "One"), hit(2, "Bob", "Two")];
  const { getByLabelText } = render(JumpToPerson);
  const input = getByLabelText("Search people");
  await fireEvent.keyDown(input, { key: "ArrowDown" }); // 0 → 1
  await fireEvent.keyDown(input, { key: "Enter" });
  expect(selection.selectPerson).toHaveBeenCalledWith(2);
});

test("Escape closes the palette", async () => {
  searchPalette.open();
  const { getByLabelText, queryByRole } = render(JumpToPerson);
  await fireEvent.keyDown(getByLabelText("Search people"), { key: "Escape" });
  expect(queryByRole("dialog")).toBeNull();
});
