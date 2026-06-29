// TreeView.modeswitch.test.ts — drive the REAL chart store through a mode switch
// (view → setMode → setMode) and assert the rendered canvas re-frames and
// re-positions. This is the closest headless reproduction of the reported bug:
// "ancestors stacked; hourglass == descendants". api + the d3 zoom action are
// mocked; everything else (store, TreeView, TreeCanvas, TreeNode) is real.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

import type { ChartMode, LayoutModel } from "../lib/types";

vi.mock("../lib/actions/zoom", () => ({
  zoomable: vi.fn(),
  prefersReducedMotion: vi.fn(() => false),
}));
vi.mock("../lib/api", () => ({ computeLayout: vi.fn() }));

import * as api from "../lib/api";
import { chart } from "../lib/stores/chart.svelte";
import { viewBoxFor } from "../lib/viewport";
import TreeView from "./TreeView.svelte";

// One person node carrying a mode + a distinctive coordinate, so each mode's
// render is unambiguous.
const modelFor = (mode: ChartMode, x: number, y: number, w: number, h: number): LayoutModel => ({
  mode,
  bounds: { x, y, width: w, height: h },
  nodes: [
    {
      id: 0,
      kind: "Person",
      entity: { Person: 11 },
      x,
      y,
      width: 220,
      height: 72,
      focal: true,
      content: {
        display_name: "Olav Lund",
        lifespan: null,
        sex: "Male",
        living: false,
        portrait: null,
      },
    },
  ],
  links: [],
});

const DESC = modelFor("Descendants", -110, 0, 976, 616);
const ANC = modelFor("Ancestors", 390, 864, 1220, 936);
const HOUR = modelFor("Hourglass", -110, 0, 1220, 1480); // bounds.y negative in reality; focus at 0
const NET = modelFor("Network", -50, 0, 1800, 1100); // whole-graph layout, wide bounds

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  vi.useRealTimers();
  vi.mocked(api.computeLayout).mockReset();
  chart.clear();
});

test("switching modes re-fetches and re-frames the canvas (ancestors ≠ descendants ≠ hourglass)", async () => {
  vi.mocked(api.computeLayout).mockImplementation(async (_root, mode) => {
    if (mode === "Ancestors") return ANC;
    if (mode === "Hourglass") return HOUR;
    return DESC;
  });

  const { container } = render(TreeView);

  // Enter the chart on a person → Descendants.
  chart.view(11);
  await flush();
  let svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(DESC.bounds, 48));
  expect(container.querySelector("g.card")?.getAttribute("transform")).toBe("translate(-110 0)");

  // Switch to Ancestors.
  chart.setMode("Ancestors");
  await flush();
  svg = container.querySelector("svg");
  expect(api.computeLayout).toHaveBeenLastCalledWith(11, "Ancestors", 4);
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(ANC.bounds, 48));
  expect(container.querySelector("g.card")?.getAttribute("transform")).toBe("translate(390 864)");

  // Switch to Hourglass.
  chart.setMode("Hourglass");
  await flush();
  svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(HOUR.bounds, 48));
  // Hourglass viewBox must differ from Descendants (else it "looks identical").
  expect(svg?.getAttribute("viewBox")).not.toBe(viewBoxFor(DESC.bounds, 48));
});

test("Network is offered, re-fetches, re-frames, and disables the depth slider", async () => {
  vi.mocked(api.computeLayout).mockImplementation(async (_root, mode) => {
    if (mode === "Network") return NET;
    return DESC;
  });

  const { container, getByRole } = render(TreeView);
  chart.view(11); // Descendants
  await flush();

  const depth = container.querySelector('input[type="range"]') as HTMLInputElement;
  expect(depth.disabled).toBe(false);

  // Click the Network button (it must exist as a fourth mode).
  await fireEvent.click(getByRole("button", { name: "Network" }));
  await flush();

  expect(api.computeLayout).toHaveBeenLastCalledWith(11, "Network", 4);
  const svg = container.querySelector("svg");
  expect(svg?.getAttribute("viewBox")).toBe(viewBoxFor(NET.bounds, 48));
  // Depth no longer applies in Network — the slider is disabled.
  expect(depth.disabled).toBe(true);
});
