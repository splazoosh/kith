// ImportSummaryDialog.test.ts — jsdom tests for the import surface (GEDCOM + LB).
// Two halves:
//   (1) the ImportSummaryDialog component — renders the counts, shows the
//       "Skipped … SOUR×2" line only when skipped_tags is non-empty, and closes on
//       Done / Esc;
//   (2) the dbActions import flows (new-tree model) — open() picks the source file,
//       save() picks the new .db, api.import{Gedcom,Lb} creates+opens it, db.adopt
//       switches to it, and the summary is surfaced via the importSummary store; either
//       cancel is a silent no-op; a failure toasts and leaves the open DB untouched.
// The engines' actual records are the Rust suites' job; this asserts the FLOW (which
// fn, which args) and the dialog's render, not import output.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

const { open, save } = vi.hoisted(() => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open, save }));
vi.mock("../lib/api", () => ({ importGedcom: vi.fn(), importLb: vi.fn() }));
vi.mock("../lib/stores/db.svelte", () => ({ db: { adopt: vi.fn() } }));

import * as api from "../lib/api";
import { importGedcom, importLb } from "../lib/dbActions";
import { CommandError } from "../lib/errors";
import { db } from "../lib/stores/db.svelte";
import { importSummary } from "../lib/stores/importSummary.svelte";
import { toast } from "../lib/stores/toast.svelte";
import type { ImportSummary } from "../lib/types";
import ImportSummaryDialog from "./ImportSummaryDialog.svelte";

const apiImport = vi.mocked(api.importGedcom);
const apiImportLb = vi.mocked(api.importLb);
const adopt = vi.mocked(db.adopt);

const SUMMARY: ImportSummary = {
  individuals: 2,
  families: 1,
  events: 3,
  names: 0,
  places: 1,
  skipped_tags: { SOUR: 2 },
};
const RESULT = { db: { path: "C:/out/tree.db", schema_version: 1 }, summary: SUMMARY };

beforeEach(() => {
  open.mockReset();
  save.mockReset();
  apiImport.mockReset();
  apiImportLb.mockReset();
  adopt.mockReset();
  toast.items = [];
  importSummary.clear();
});

// — (1) the component —

test("renders the imported counts", () => {
  const { getByText } = render(ImportSummaryDialog, {
    props: { summary: SUMMARY, onclose: vi.fn() },
  });
  expect(getByText("2 individuals")).toBeTruthy();
  expect(getByText("1 families")).toBeTruthy();
  expect(getByText("3 events")).toBeTruthy();
  expect(getByText("0 alternate names")).toBeTruthy();
  expect(getByText("1 places")).toBeTruthy();
});

test("shows the skipped-records line when records were skipped", () => {
  const { getByText } = render(ImportSummaryDialog, {
    props: { summary: SUMMARY, onclose: vi.fn() },
  });
  expect(getByText(/Skipped unsupported records: SOUR×2/)).toBeTruthy();
});

test("omits the skipped-records line on a clean import", () => {
  const { queryByText } = render(ImportSummaryDialog, {
    props: {
      summary: { ...SUMMARY, skipped_tags: {} },
      onclose: vi.fn(),
    },
  });
  expect(queryByText(/Skipped unsupported records/)).toBeNull();
});

test("clicking Done calls onclose", async () => {
  const onclose = vi.fn();
  const { getByRole } = render(ImportSummaryDialog, {
    props: { summary: SUMMARY, onclose },
  });
  await fireEvent.click(getByRole("button", { name: "Done" }));
  expect(onclose).toHaveBeenCalledOnce();
});

test("pressing Escape calls onclose", async () => {
  const onclose = vi.fn();
  render(ImportSummaryDialog, { props: { summary: SUMMARY, onclose } });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onclose).toHaveBeenCalledOnce();
});

// — (2) the dbActions.importGedcom flow (new-tree model) —

test("imports into a new tree: picks ged + db, opens it, and surfaces the summary", async () => {
  open.mockResolvedValue("C:/in/tree.ged");
  save.mockResolvedValue("C:/out/tree.db");
  apiImport.mockResolvedValue(RESULT);

  const result = await importGedcom();

  expect(apiImport).toHaveBeenCalledWith("C:/in/tree.ged", "C:/out/tree.db");
  expect(adopt).toHaveBeenCalledWith(RESULT.db);
  expect(importSummary.current).toEqual(SUMMARY);
  expect(result).toEqual(SUMMARY);
});

test("seeds the destination filename from the chosen GEDCOM's basename", async () => {
  open.mockResolvedValue("C:/in/family.ged");
  save.mockResolvedValue(null); // cancel after seeing the default

  await importGedcom();

  expect(save).toHaveBeenCalledWith(
    expect.objectContaining({ defaultPath: "family.db" }),
  );
});

test("cancelling the file picker returns null with no destination prompt", async () => {
  open.mockResolvedValue(null);

  const result = await importGedcom();

  expect(save).not.toHaveBeenCalled();
  expect(apiImport).not.toHaveBeenCalled();
  expect(result).toBeNull();
});

test("cancelling the destination picker returns null without importing", async () => {
  open.mockResolvedValue("C:/in/tree.ged");
  save.mockResolvedValue(null);

  const result = await importGedcom();

  expect(apiImport).not.toHaveBeenCalled();
  expect(adopt).not.toHaveBeenCalled();
  expect(result).toBeNull();
});

test("a failed import toasts the error and leaves the open DB untouched", async () => {
  open.mockResolvedValue("C:/in/tree.ged");
  save.mockResolvedValue("C:/out/tree.db");
  apiImport.mockRejectedValue(
    new CommandError("validation", "line 12: invalid level"),
  );

  const result = await importGedcom();

  expect(result).toBeNull();
  expect(adopt).not.toHaveBeenCalled();
  expect(importSummary.current).toBeNull();
  expect(toast.items.at(-1)).toMatchObject({
    kind: "validation",
    message: "line 12: invalid level",
    sticky: true,
  });
});

// — (3) the dbActions.importLb flow — the same new-tree machinery, over the LB engine —

test("importLb imports into a new tree: picks json + db, opens it, surfaces the summary", async () => {
  open.mockResolvedValue("C:/in/people.json");
  save.mockResolvedValue("C:/out/people.db");
  apiImportLb.mockResolvedValue({ db: RESULT.db, summary: SUMMARY });

  const result = await importLb();

  expect(apiImportLb).toHaveBeenCalledWith("C:/in/people.json", "C:/out/people.db");
  // Seeds the destination from the chosen file's basename (people.json → people.db).
  expect(save).toHaveBeenCalledWith(
    expect.objectContaining({ defaultPath: "people.db" }),
  );
  expect(adopt).toHaveBeenCalledWith(RESULT.db);
  expect(importSummary.current).toEqual(SUMMARY);
  expect(result).toEqual(SUMMARY);
});

test("importLb cancelling the file picker returns null without importing", async () => {
  open.mockResolvedValue(null);

  const result = await importLb();

  expect(save).not.toHaveBeenCalled();
  expect(apiImportLb).not.toHaveBeenCalled();
  expect(result).toBeNull();
});
