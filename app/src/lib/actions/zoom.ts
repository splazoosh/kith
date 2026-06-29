// zoom.ts — the d3-zoom ⇄ Svelte seam. d3 attaches to the <svg> and reports
// each transform back through `onzoom`; it NEVER selects/appends/data-binds a
// node (Svelte owns the DOM). `fit` resets to the identity transform — which,
// with the viewBox already framing bounds, re-frames the whole chart. The
// reset is INSTANT (not a d3 `.transition()`), which both fixes a stale-transform
// bug on model change (see `fit`) and satisfies reduced motion for free.
import { select } from "d3-selection";
import { type ZoomTransform, zoom as d3zoom, zoomIdentity } from "d3-zoom";

const MIN_SCALE = 0.2;
const MAX_SCALE = 4;

/**
 * True when the OS asks for reduced motion. Exposed for callers that animate in
 * the JS path the global CSS reset (global.css) does NOT reach; `fit` itself is
 * instant, so it needs no gate.
 */
export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export interface ZoomParams {
  /** Called on every zoom/pan with d3's transform; the component stores it. */
  onzoom: (t: ZoomTransform) => void;
  /** The action hands back a `fit()` so the component can trigger fit-to-screen. */
  register?: (handle: { fit: () => void }) => void;
}

/** Svelte action: `use:zoomable={{ onzoom, register }}` on the <svg>. */
export function zoomable(svg: SVGSVGElement, params: ZoomParams) {
  const sel = select(svg);
  const behavior = d3zoom<SVGSVGElement, unknown>()
    .scaleExtent([MIN_SCALE, MAX_SCALE])
    .on("zoom", (e: { transform: ZoomTransform }) => params.onzoom(e.transform));
  sel.call(behavior);

  const fit = (): void => {
    // Reset to the identity transform — with the viewBox already framing `bounds`,
    // identity re-frames the whole chart. Done INSTANTLY (not via a d3
    // `.transition()`): a transition does NOT reliably drive the zoom handler
    // here, so on a model change (mode switch / re-root) it left the *previous*
    // pan/zoom in place and mis-framed the new chart. `selection.call(transform)`
    // runs the gesture synchronously, firing `onzoom` so the stored transform
    // resets deterministically. (Instant also satisfies reduced-motion for free.)
    sel.call(behavior.transform, zoomIdentity);
  };
  params.register?.({ fit });

  return {
    destroy() {
      sel.on(".zoom", null); // tear the listeners down with the component
    },
  };
}
