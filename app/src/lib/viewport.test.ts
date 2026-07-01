// viewport.test.ts — the two pure geometry helpers (the ONLY geometry the canvas
// computes) plus the reduced-motion probe. jsdom does not implement
// `window.matchMedia`, so the prefersReducedMotion case stubs it and asserts it
// is consulted (the JS path the global CSS reset cannot reach).

import { afterEach, expect, test, vi } from "vitest";

import { prefersReducedMotion } from "./actions/zoom";
import { pathFromAnchors, placeFor, roundedPathFromAnchors, viewBoxFor } from "./viewport";

afterEach(() => {
  vi.unstubAllGlobals();
});

test("viewBoxFor expands the tight bounds by the margin on every side", () => {
  // Arrange — bounds with a negative x (an ancestors/hourglass chart reflects up).
  const bounds = { x: -110, y: 0, width: 220, height: 152 };
  // Act + Assert — x-m, y-m, w+2m, h+2m.
  expect(viewBoxFor(bounds, 48)).toBe("-158 -48 316 248");
});

test("pathFromAnchors strokes a straight polyline through the given waypoints", () => {
  expect(pathFromAnchors([{ x: 0, y: 72 }, { x: 0, y: 136 }])).toBe(
    "M 0 72 L 0 136",
  );
});

test("pathFromAnchors yields an empty path for no anchors", () => {
  expect(pathFromAnchors([])).toBe("");
});

test("roundedPathFromAnchors with radius 0 is exactly the straight path (additive)", () => {
  const a = [{ x: 0, y: 0 }, { x: 0, y: 50 }, { x: 40, y: 50 }];
  expect(roundedPathFromAnchors(a, 0)).toBe(pathFromAnchors(a));
});

test("roundedPathFromAnchors with ≤2 anchors is the straight path (no elbow to round)", () => {
  const a = [{ x: 0, y: 0 }, { x: 0, y: 50 }];
  expect(roundedPathFromAnchors(a, 10)).toBe(pathFromAnchors(a));
});

test("roundedPathFromAnchors rounds a 3-anchor elbow with an L … Q … corner", () => {
  // Trim 25 back along the vertical into the elbow, quadratic AT the corner,
  // 25 along the horizontal out — over the model's OWN waypoints, never rerouted.
  const a = [{ x: 0, y: 0 }, { x: 0, y: 100 }, { x: 100, y: 100 }];
  expect(roundedPathFromAnchors(a, 25)).toBe("M 0 0 L 0 75 Q 0 100 25 100 L 100 100");
});

test("roundedPathFromAnchors clamps the trim to half a short segment", () => {
  // The middle segment is length 10; an 8-radius trim is clamped to 5 (half) so
  // adjacent short segments can't overshoot into a self-crossing corner.
  const a = [{ x: 0, y: 0 }, { x: 0, y: 10 }, { x: 10, y: 10 }];
  expect(roundedPathFromAnchors(a, 8)).toBe("M 0 0 L 0 5 Q 0 10 5 10 L 10 10");
});

test("roundedPathFromAnchors yields an empty path for no anchors", () => {
  expect(roundedPathFromAnchors([], 10)).toBe("");
});

// — placeFor: the pure detail-popover placement (right / flip-left / clamp). All
//   rects are container-relative px; the container origin is (0, 0). —
const container = { left: 0, top: 0, width: 800, height: 600 };

test("placeFor puts the popover to the right of the anchor when there is room", () => {
  const anchor = { left: 100, top: 50, width: 200, height: 80 };
  // 100 + 200 + gap(8) = 308; 308 + 240 = 548 ≤ 800 → stays right.
  expect(placeFor(anchor, { width: 240, height: 300 }, container, 8)).toEqual({
    left: 308,
    top: 50,
  });
});

test("placeFor flips the popover left when the right edge would overflow", () => {
  const anchor = { left: 600, top: 50, width: 200, height: 80 };
  // right = 800 + 8 + 240 = 1048 > 800 → flip: 600 - 8 - 240 = 352.
  expect(placeFor(anchor, { width: 240, height: 300 }, container, 8)).toEqual({
    left: 352,
    top: 50,
  });
});

test("placeFor clamps the top up when the anchor sits near the bottom", () => {
  const anchor = { left: 100, top: 550, width: 200, height: 80 };
  // top 550 + 300 = 850 > 600 → clamp to 600 - 300 = 300.
  expect(placeFor(anchor, { width: 240, height: 300 }, container, 8).top).toBe(300);
});

test("placeFor clamps the top to 0 when the popover is taller than the container", () => {
  const short = { left: 0, top: 0, width: 800, height: 200 };
  const anchor = { left: 100, top: 400, width: 200, height: 80 };
  expect(placeFor(anchor, { width: 240, height: 300 }, short, 8).top).toBe(0);
});

test("placeFor clamps a flip-left that would run off the left edge to 0", () => {
  const narrow = { left: 0, top: 0, width: 300, height: 600 };
  const anchor = { left: 120, top: 20, width: 160, height: 80 };
  // right = 280 + 8 + 260 = 548 > 300 → flip: 120 - 8 - 260 = -148 → clamp to 0.
  expect(placeFor(anchor, { width: 260, height: 200 }, narrow, 8).left).toBe(0);
});

test("prefersReducedMotion consults window.matchMedia and returns its match", () => {
  // Arrange — a stub standing in for the (jsdom-absent) matchMedia.
  const matchMedia = vi.fn().mockReturnValue({ matches: true });
  vi.stubGlobal("matchMedia", matchMedia);

  // Act + Assert — the helper reads the reduce query and surfaces its `matches`.
  expect(prefersReducedMotion()).toBe(true);
  expect(matchMedia).toHaveBeenCalledWith("(prefers-reduced-motion: reduce)");
});
