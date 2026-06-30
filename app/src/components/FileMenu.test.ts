// FileMenu.test.ts — the consolidated "File ▾" dropdown. Its actions (dbActions /
// exportActions) and the db store are mocked, so this asserts the MENU behaviour:
// the trigger opens it, the file/import items (including the new LB import) are
// present, Export/Close appear only with a database open, and choosing an item runs
// the action and collapses the menu. The actions' own flows are tested elsewhere.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

const { dbMock, actions } = vi.hoisted(() => ({
  dbMock: {
    current: null as null | { path: string; schema_version: number },
    close: vi.fn(),
  },
  actions: {
    pickAndOpen: vi.fn(),
    pickAndCreate: vi.fn(),
    importGedcom: vi.fn(),
    importLb: vi.fn(),
    exportGedcom: vi.fn(),
  },
}));

vi.mock("../lib/stores/db.svelte", () => ({ db: dbMock }));
vi.mock("../lib/dbActions", () => ({
  pickAndOpen: actions.pickAndOpen,
  pickAndCreate: actions.pickAndCreate,
  importGedcom: actions.importGedcom,
  importLb: actions.importLb,
}));
vi.mock("../lib/exportActions", () => ({ exportGedcom: actions.exportGedcom }));

import FileMenu from "./FileMenu.svelte";

beforeEach(() => {
  dbMock.current = null;
  dbMock.close.mockReset();
  Object.values(actions).forEach((m) => m.mockReset());
});

/** Render and open the menu; returns the testing-library queries. */
async function openMenu() {
  const utils = render(FileMenu);
  await fireEvent.click(utils.getByRole("button", { name: /file/i }));
  return utils;
}

test("opening the menu lists the file + import items, including LB import", async () => {
  const { getByText, queryByText } = await openMenu();

  expect(getByText("Open…")).toBeTruthy();
  expect(getByText("Create…")).toBeTruthy();
  expect(getByText("Import GEDCOM…")).toBeTruthy();
  expect(getByText("Import LB (JSON)…")).toBeTruthy();

  // With no database open, there is nothing to export or close.
  expect(queryByText("Export GEDCOM…")).toBeNull();
  expect(queryByText("Close database")).toBeNull();
});

test("Export and Close appear only when a database is open", async () => {
  dbMock.current = { path: "C:/x/tree.db", schema_version: 1 };
  const { getByText } = await openMenu();

  expect(getByText("Export GEDCOM…")).toBeTruthy();
  expect(getByText("Close database")).toBeTruthy();
});

test("choosing Import LB runs the action and collapses the menu", async () => {
  const { getByText, queryByText } = await openMenu();

  await fireEvent.click(getByText("Import LB (JSON)…"));

  expect(actions.importLb).toHaveBeenCalledOnce();
  // The menu collapses after a choice (its item is gone from the DOM).
  expect(queryByText("Import LB (JSON)…")).toBeNull();
});

test("the trigger toggles the menu open and closed", async () => {
  const { getByRole, queryByText } = render(FileMenu);
  const trigger = getByRole("button", { name: /file/i });

  await fireEvent.click(trigger);
  expect(queryByText("Open…")).not.toBeNull();
  expect(trigger.getAttribute("aria-expanded")).toBe("true");

  await fireEvent.click(trigger);
  expect(queryByText("Open…")).toBeNull();
  expect(trigger.getAttribute("aria-expanded")).toBe("false");
});
