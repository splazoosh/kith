// undo.svelte.test.ts — the session undo store. `api` and the
// refresh-target stores are mocked (`ui` is real, so the Tree-only chart re-fetch
// is exercised). The store only marshals the `undo` command + refreshes views —
// these tests pin recordDelete, the depth sync, the view refresh keyed on `kind`,
// and the null / error paths.

import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../api", () => ({ undo: vi.fn() }));
vi.mock("./toast.svelte", () => ({
  toast: { pushAction: vi.fn(), pushNotice: vi.fn(), pushError: vi.fn() },
}));
vi.mock("./library.svelte", () => ({ library: { reload: vi.fn() } }));
vi.mock("./selection.svelte", () => ({ selection: { reselect: vi.fn() } }));
vi.mock("./sources.svelte", () => ({ sources: { reload: vi.fn() } }));
vi.mock("./chart.svelte", () => ({
  chart: { view: vi.fn(), rootId: null as number | null },
}));

import * as api from "../api";
import { CommandError } from "../errors";
import { chart } from "./chart.svelte";
import { library } from "./library.svelte";
import { selection } from "./selection.svelte";
import { sources } from "./sources.svelte";
import { toast } from "./toast.svelte";
import { ui } from "./ui.svelte";
import { undo } from "./undo.svelte";

beforeEach(() => {
  vi.mocked(api.undo).mockReset();
  vi.mocked(toast.pushAction).mockReset();
  vi.mocked(toast.pushNotice).mockReset();
  vi.mocked(toast.pushError).mockReset();
  vi.mocked(library.reload).mockReset().mockResolvedValue(undefined);
  vi.mocked(selection.reselect).mockReset().mockResolvedValue(undefined);
  vi.mocked(sources.reload).mockReset().mockResolvedValue(undefined);
  vi.mocked(chart.view).mockReset();
  (chart as { rootId: number | null }).rootId = null;
  ui.showLibrary();
  undo.reset();
});

test("recordDelete bumps the depth and shows the action toast", () => {
  undo.recordDelete("Jane Doe");
  expect(undo.depth).toBe(1);
  expect(undo.topLabel).toBe("Jane Doe");
  expect(toast.pushAction).toHaveBeenCalledWith(
    "Deleted Jane Doe",
    "Undo",
    expect.any(Function),
  );
});

test("runUndo restores, confirms, syncs the depth, and refreshes the library", async () => {
  undo.recordDelete("Jane");
  vi.mocked(api.undo).mockResolvedValue({
    kind: "person",
    label: "Jane Doe",
    remaining: 0,
  });
  await undo.runUndo();
  expect(api.undo).toHaveBeenCalledOnce();
  expect(undo.depth).toBe(0);
  expect(undo.topLabel).toBeNull();
  expect(toast.pushNotice).toHaveBeenCalledWith("Restored Jane Doe");
  expect(library.reload).toHaveBeenCalled();
  expect(selection.reselect).toHaveBeenCalled();
});

test("runUndo on an empty stack is a no-op (no command call)", async () => {
  await undo.runUndo();
  expect(api.undo).not.toHaveBeenCalled();
});

test("a null outcome (nothing left server-side) resets the depth", async () => {
  undo.recordDelete("X");
  vi.mocked(api.undo).mockResolvedValue(null);
  await undo.runUndo();
  expect(undo.depth).toBe(0);
  expect(toast.pushNotice).not.toHaveBeenCalled();
});

test("a restore error decrements the depth and toasts (the reused-id path)", async () => {
  undo.recordDelete("X");
  undo.recordDelete("Y"); // depth 2
  vi.mocked(api.undo).mockRejectedValue(
    new CommandError("database", "that record's id was reused"),
  );
  await undo.runUndo();
  expect(undo.depth).toBe(1);
  expect(toast.pushError).toHaveBeenCalledOnce();
});

test("a source undo reloads the sources catalogue", async () => {
  undo.recordDelete("Bergen Register");
  vi.mocked(api.undo).mockResolvedValue({
    kind: "source",
    label: "Bergen Register",
    remaining: 0,
  });
  await undo.runUndo();
  expect(sources.reload).toHaveBeenCalled();
});

test("undo re-fetches the chart only when the Tree view is active", async () => {
  undo.recordDelete("X");
  ui.showTree();
  (chart as { rootId: number | null }).rootId = 5;
  vi.mocked(api.undo).mockResolvedValue({
    kind: "person",
    label: "X",
    remaining: 0,
  });
  await undo.runUndo();
  expect(chart.view).toHaveBeenCalledWith(5);
});
