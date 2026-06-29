<script lang="ts">
  // TreeCanvas — the render heart. Turns a LayoutModel into elegant SVG and
  // NOTHING more: the only geometry is the viewBox from bounds and each
  // link's stroke through its given anchors, now rounded over the SAME waypoints.
  // d3 owns ONLY the transform on the single root <g>. A node
  // click re-roots via the onreroot prop; the fit handle is surfaced to the
  // toolbar via onready. Prop-driven + store-agnostic.
  import type { ZoomTransform } from "d3-zoom";

  import { zoomable, type ZoomParams } from "../lib/actions/zoom";
  import type { LayoutModel } from "../lib/types";
  import { roundedPathFromAnchors, viewBoxFor } from "../lib/viewport";
  import TreeNode from "./TreeNode.svelte";

  interface Props {
    model: LayoutModel;
    /** MediaId → asset URL, resolved by the caller (the media store). A node's
     *  portrait is drawn only when its id resolves here. Default empty. */
    portraitUrls?: Record<number, string>;
    onreroot: (personId: number) => void;
    onready?: (handle: { fit: () => void }) => void;
  }
  let { model, portraitUrls = {}, onreroot, onready }: Props = $props();

  /** The asset URL for a node's portrait, or null (store-agnostic: a plain map). */
  function portraitOf(node: LayoutModel["nodes"][number]): string | null {
    const id = node.content?.portrait;
    return id != null ? (portraitUrls[id] ?? null) : null;
  }

  const MARGIN = 48; // viewport breathing room — applied to the viewBox, not geometry
  const CORNER = 10; // rounded-elbow radius over the model's anchors
  let transform = $state({ x: 0, y: 0, k: 1 });
  let fit: (() => void) | null = null;

  const onzoom = (t: ZoomTransform): void => {
    transform = { x: t.x, y: t.y, k: t.k };
  };
  const register = (h: { fit: () => void }): void => {
    fit = h.fit;
    onready?.(h); // surface fit to the toolbar
  };

  // Re-frame whenever the model changes (re-root / mode / depth) — this is the
  // re-root recenter, reduced-motion-aware via the action's fit.
  $effect(() => {
    void model;
    fit?.();
  });
</script>

<svg
  class="canvas"
  width="100%"
  height="100%"
  viewBox={viewBoxFor(model.bounds, MARGIN)}
  preserveAspectRatio="xMidYMid meet"
  role="group"
  aria-label="Family tree chart"
  use:zoomable={{ onzoom, register } satisfies ZoomParams}
>
  <defs>
    <!-- A single shared card shadow — CSS box-shadow does NOT apply to SVG. -->
    <filter id="card-shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="1" stdDeviation="1.5" flood-opacity="0.18" />
    </filter>
  </defs>
  <g transform={`translate(${transform.x} ${transform.y}) scale(${transform.k})`}>
    {#each model.links as link, i (i)}
      <path
        class="link {link.kind.toLowerCase()}"
        d={roundedPathFromAnchors(link.anchors, CORNER)}
        fill="none"
      />
    {/each}
    {#each model.nodes as node (node.id)}
      <TreeNode {node} portraitUrl={portraitOf(node)} {onreroot} />
    {/each}
  </g>
</svg>

<style>
  .canvas {
    display: block;
    width: 100%;
    height: 100%;
    background: var(--color-paper);
    touch-action: none; /* let d3-zoom own the gesture */
  }
  .link {
    stroke: var(--tree-link);
    stroke-width: 1.5;
  }
  /* Partner links read with restraint — a quiet dash distinguishes them. */
  .link.partner {
    stroke-dasharray: 5 4;
  }
</style>
