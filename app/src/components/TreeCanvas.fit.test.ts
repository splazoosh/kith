// TreeCanvas.fit.test.ts — the re-fit-on-model-change contract (the
// mode-switch bug). Unlike TreeCanvas.test.ts (which mocks the zoom action to a
// no-op), this mock wires the action's `register({ fit })` to a spy, so we can
// assert the canvas re-frames (calls fit) every time the model is swapped — which
// is what a mode switch / re-root does. If fit fires only once (on mount), a
// prior pan/zoom persists across the switch and the new chart is mis-framed.

import { render } from "@testing-library/svelte";
import { expect, test, vi } from "vitest";

const fitSpy = vi.hoisted(() => vi.fn());

vi.mock("../lib/actions/zoom", () => ({
  zoomable: (_svg: SVGSVGElement, params: { register?: (h: { fit: () => void }) => void }) => {
    params.register?.({ fit: fitSpy });
    return { destroy() {} };
  },
  prefersReducedMotion: () => false,
}));

import type { LayoutModel } from "../lib/types";
import TreeCanvas from "./TreeCanvas.svelte";

const noop = (): void => {};

const lone = (mode: LayoutModel["mode"], x: number): LayoutModel => ({
  mode,
  bounds: { x, y: 0, width: 220, height: 72 },
  nodes: [
    {
      id: 0,
      kind: "Person",
      entity: { Person: 1 },
      x,
      y: 0,
      width: 220,
      height: 72,
      focal: true,
      content: {
        display_name: "Focus",
        lifespan: null,
        sex: "Male",
        living: false,
        portrait: null,
      },
    },
  ],
  links: [],
});

test("fit runs on mount AND again whenever the model is swapped (mode switch / re-root)", async () => {
  fitSpy.mockClear();

  const { rerender } = render(TreeCanvas, {
    props: { model: lone("Descendants", 0), onreroot: noop },
  });
  expect(fitSpy).toHaveBeenCalledTimes(1); // mount

  await rerender({ model: lone("Ancestors", 500), onreroot: noop });
  expect(fitSpy).toHaveBeenCalledTimes(2); // re-framed for the new model

  await rerender({ model: lone("Hourglass", -500), onreroot: noop });
  expect(fitSpy).toHaveBeenCalledTimes(3);
});
