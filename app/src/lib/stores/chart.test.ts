// chart.test.ts — the reactive driver. `api` and the toast
// channel are mocked; `errors` is real, so the failure path exercises the actual
// asCommandError pass-through. The store STORES a model — it never builds one:
// every control mutates (rootId, mode, generations) and re-fetches via the one
// private #load(). These tests pin the re-fetch tuple, the debounce, the
// out-of-order guard, and the error path.

import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../api", () => ({ computeLayout: vi.fn() }));
vi.mock("./toast.svelte", () => ({ toast: { pushError: vi.fn() } }));

import * as api from "../api";
import { CommandError } from "../errors";
import type { ChartMode, LayoutModel } from "../types";
import { chart } from "./chart.svelte";
import { toast } from "./toast.svelte";

const EMPTY: LayoutModel = {
  mode: "Descendants",
  nodes: [],
  links: [],
  bounds: { x: 0, y: 0, width: 0, height: 0 },
};
const modelWith = (mode: ChartMode): LayoutModel => ({ ...EMPTY, mode });

// Drain microtasks + the macrotask queue so an awaited #load() settles.
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.useRealTimers();
  vi.mocked(api.computeLayout).mockReset().mockResolvedValue(EMPTY);
  vi.mocked(toast.pushError).mockReset();
  chart.clear();
});

test("view loads the root with the current mode + depth", () => {
  chart.view(1);
  expect(chart.rootId).toBe(1);
  expect(api.computeLayout).toHaveBeenCalledWith(1, "Descendants", 4);
});

test("view stores the returned model and clears loading", async () => {
  vi.mocked(api.computeLayout).mockResolvedValue(modelWith("Hourglass"));
  chart.view(1);
  await flush();
  expect(chart.model?.mode).toBe("Hourglass");
  expect(chart.loading).toBe(false);
  expect(chart.error).toBeNull();
});

test("reroot changes only the root (mode + depth unchanged)", () => {
  chart.view(1);
  chart.reroot(2);
  expect(chart.rootId).toBe(2);
  expect(chart.mode).toBe("Descendants");
  expect(chart.generations).toBe(4);
  expect(api.computeLayout).toHaveBeenLastCalledWith(2, "Descendants", 4);
});

test("reroot on the current root is a no-op", () => {
  chart.view(1);
  vi.mocked(api.computeLayout).mockClear();
  chart.reroot(1);
  expect(api.computeLayout).not.toHaveBeenCalled();
});

test("setMode re-fetches with the new mode, keeping root + depth", () => {
  chart.view(1);
  chart.setMode("Ancestors");
  expect(chart.mode).toBe("Ancestors");
  expect(api.computeLayout).toHaveBeenLastCalledWith(1, "Ancestors", 4);
});

test("setGenerations debounces a slider drag into one fetch", () => {
  vi.useFakeTimers();
  chart.view(1); // immediate
  vi.mocked(api.computeLayout).mockClear();
  chart.setGenerations(5);
  chart.setGenerations(6);
  chart.setGenerations(7);
  vi.advanceTimersByTime(200);
  expect(api.computeLayout).toHaveBeenCalledTimes(1);
  expect(api.computeLayout).toHaveBeenCalledWith(1, "Descendants", 7);
});

test("an out-of-order resolve does not clobber a newer load", async () => {
  const slow = deferred<LayoutModel>();
  vi.mocked(api.computeLayout)
    .mockReturnValueOnce(slow.promise) // view(1) — seq 1, resolves LATER
    .mockResolvedValueOnce(modelWith("Ancestors")); // reroot(2) — seq 2, resolves now

  chart.view(1); // starts the slow load
  chart.reroot(2); // starts the fast load, superseding it
  await flush();
  expect(chart.model?.mode).toBe("Ancestors"); // the newer (fast) load won

  slow.resolve(modelWith("Hourglass")); // the stale load finally resolves
  await flush();
  expect(chart.model?.mode).toBe("Ancestors"); // …and is dropped, not applied
});

test("a failed load sets error, nulls the model, and toasts", async () => {
  vi.mocked(api.computeLayout).mockRejectedValueOnce(
    new CommandError("not_found", "no individual 9"),
  );
  chart.view(9);
  await flush();
  expect(chart.error).toBe("no individual 9");
  expect(chart.model).toBeNull();
  expect(chart.loading).toBe(false);
  expect(toast.pushError).toHaveBeenCalledOnce();
});

test("clear resets params and invalidates an in-flight load", async () => {
  const slow = deferred<LayoutModel>();
  vi.mocked(api.computeLayout).mockReturnValueOnce(slow.promise);
  chart.view(1);
  chart.clear();
  expect(chart.rootId).toBeNull();
  expect(chart.mode).toBe("Descendants");
  expect(chart.generations).toBe(4);

  slow.resolve(modelWith("Ancestors")); // a load mid-clear must not resurrect a chart
  await flush();
  expect(chart.model).toBeNull();
});
