// viewport.ts — the only geometry the canvas computes. A declarative
// re-frame of the model's bounds into an SVG viewBox, and a stroke path straight
// through the model's routing anchors. NO node is positioned here, NO link is
// routed here — the model owns both; this only frames and strokes what it gives.
import type { Point, Rect } from "./types";

/**
 * Expand the tight `bounds` by `margin` (viewport breathing room, NOT baked into
 * geometry) and format as an SVG `viewBox` string. The browser's
 * `preserveAspectRatio` then owns the bounds→viewport letterbox math, so no
 * bounds→pixel transform is ever recomputed in JS.
 */
export function viewBoxFor(bounds: Rect, margin: number): string {
  const x = bounds.x - margin;
  const y = bounds.y - margin;
  const w = bounds.width + margin * 2;
  const h = bounds.height + margin * 2;
  return `${x} ${y} ${w} ${h}`;
}

/**
 * A polyline `d` straight through the model's routing anchors. The anchors
 * already encode the orthogonal route; the renderer chooses only the stroke
 * between waypoints (rounded elbows are a separate pass over these same points —
 * never a re-route). An empty anchor list yields an empty path.
 */
export function pathFromAnchors(anchors: Point[]): string {
  if (anchors.length === 0) return "";
  const [head, ...rest] = anchors;
  return `M ${head.x} ${head.y}` + rest.map((p) => ` L ${p.x} ${p.y}`).join("");
}

/**
 * A polyline `d` with rounded interior elbows over the model's OWN anchors.
 * `radius` trims each adjacent segment and joins them with a quadratic
 * corner; `radius === 0` (or ≤ 2 anchors) is exactly the straight
 * {@link pathFromAnchors} output, so the rounding is purely additive. NO
 * re-route — the waypoints are the model's; this only shapes the stroke.
 */
export function roundedPathFromAnchors(anchors: Point[], radius: number): string {
  if (anchors.length === 0) return "";
  if (anchors.length <= 2 || radius <= 0) return pathFromAnchors(anchors);

  let d = `M ${anchors[0].x} ${anchors[0].y}`;
  for (let i = 1; i < anchors.length - 1; i++) {
    const prev = anchors[i - 1];
    const cur = anchors[i];
    const next = anchors[i + 1];
    const a = trimToward(cur, prev, radius); // a point `radius` back along cur→prev
    const b = trimToward(cur, next, radius); // a point `radius` along cur→next
    d += ` L ${a.x} ${a.y} Q ${cur.x} ${cur.y} ${b.x} ${b.y}`;
  }
  const last = anchors[anchors.length - 1];
  return `${d} L ${last.x} ${last.y}`;
}

/**
 * A point `dist` from `from` toward `to`, clamped to the segment's half-length
 * so two short adjacent segments can't overshoot into a self-crossing corner.
 */
function trimToward(from: Point, to: Point, dist: number): Point {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const len = Math.hypot(dx, dy);
  if (len === 0) return { x: from.x, y: from.y };
  const t = Math.min(dist, len / 2) / len;
  return { x: from.x + dx * t, y: from.y + dy * t };
}

/** A screen rectangle (container-relative px) — the detail popover's placement input. */
export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Position a popover of `size` beside `anchor`, kept inside `container`. Every
 * rect is in the SAME coordinate space (container-relative px, so `anchor` is the
 * node's `getBoundingClientRect` minus the container's origin). Prefers the right
 * of the anchor; flips left when the right would overflow; aligns the popover's
 * top to the anchor's top, then clamps both axes inside the container so a corner
 * card never pushes it off-screen. Returns container-relative `{ left, top }`.
 *
 * This is pure screen-rect arithmetic — deliberately NO model/viewBox/transform
 * math (that stays in the core; a tooltip anchor is interaction chrome, not
 * layout). The reader takes the anchor rect from the rendered DOM.
 */
export function placeFor(
  anchor: Box,
  size: { width: number; height: number },
  container: Box,
  gap = 8,
): { left: number; top: number } {
  const rightEdge = anchor.left + anchor.width;
  // Prefer the right of the anchor; flip left if it would overflow the container.
  let left = rightEdge + gap;
  if (left + size.width > container.width) {
    left = anchor.left - gap - size.width;
  }
  // Clamp both axes so a card near an edge/corner never pushes it off-screen.
  return {
    left: clamp(left, 0, Math.max(0, container.width - size.width)),
    top: clamp(anchor.top, 0, Math.max(0, container.height - size.height)),
  };
}

/** Clamp `value` into the inclusive `[min, max]` range. */
function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
