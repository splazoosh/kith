<script lang="ts">
  // TreeCanvas — the render heart. Turns a LayoutModel into elegant SVG: the only
  // geometry is the viewBox from bounds and each link's stroke through its given
  // anchors, rounded over the SAME waypoints. d3 owns ONLY the transform on the
  // single root <g>. A DOUBLE click re-roots via the onreroot prop; a SINGLE click
  // (or Enter/Space) inspects — it hosts the read-only detail popover in a
  // positioned wrapper, anchored to the clicked card and following it through
  // pan/zoom. The only NEW geometry is reading a rendered rect + the pure
  // placeFor (interaction chrome, never layout math). The fit handle is surfaced
  // via onready.
  import type { ZoomTransform } from "d3-zoom";

  import { zoomable, type ZoomParams } from "../lib/actions/zoom";
  import { nodeDetail } from "../lib/stores/nodeDetail.svelte";
  import type { LayoutModel } from "../lib/types";
  import { placeFor, roundedPathFromAnchors, viewBoxFor } from "../lib/viewport";
  import DetailPopover from "./DetailPopover.svelte";
  import TreeNode from "./TreeNode.svelte";

  interface Props {
    model: LayoutModel;
    /** MediaId → asset URL, resolved by the caller (the media store). A node's
     *  portrait is drawn only when its id resolves here. Default empty. */
    portraitUrls?: Record<number, string>;
    onreroot: (personId: number) => void;
    /** Open a person in the Library (the popover's "Open in Library" action). */
    onopenlibrary?: (personId: number) => void;
    onready?: (handle: { fit: () => void }) => void;
  }
  let { model, portraitUrls = {}, onreroot, onopenlibrary, onready }: Props = $props();

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

  // — detail popover host (D3) —
  let wrapEl = $state<HTMLDivElement | null>(null);
  let popoverEl = $state<HTMLDivElement | null>(null);
  // The clicked node's rendered <g>, so the popover can anchor + follow it, and
  // so focus returns to it on close. A plain ref (not $state) — read imperatively.
  let activeEl: SVGGElement | null = null;
  let wasOpen = false; // tracks the open→closed transition for focus restore
  let pos = $state({ left: 0, top: 0 });

  /** The focal person's id (Center is hidden when inspecting the current root). */
  const focalPersonId = $derived.by((): number | null => {
    const focal = model.nodes.find((n) => n.focal);
    return focal && "Person" in focal.entity ? focal.entity.Person : null;
  });

  /** The inspected person's portrait URL, from the canvas's already-resolved map
   *  (no extra IPC — the popover reuses what the cards use). */
  const activePortraitUrl = $derived.by((): string | null => {
    const id = nodeDetail.target?.id;
    if (id == null) return null;
    const node = model.nodes.find((n) => "Person" in n.entity && n.entity.Person === id);
    const portrait = node?.content?.portrait;
    return portrait != null ? (portraitUrls[portrait] ?? null) : null;
  });

  function onInspect(personId: number, groupEl: SVGGElement): void {
    activeEl = groupEl;
    void nodeDetail.open(personId);
  }

  // Center = re-root (identical to double-click); Open in Library delegates to the
  // container. Both close first so the popover doesn't linger over the new state.
  function center(personId: number): void {
    nodeDetail.close();
    onreroot(personId);
  }
  function openInLibrary(personId: number): void {
    nodeDetail.close();
    onopenlibrary?.(personId);
  }

  // Outside-click on the canvas background closes; a node click re-targets (its
  // own debounced handler runs), and a click inside the popover is ignored.
  function onWrapClick(e: MouseEvent): void {
    if (nodeDetail.target === null) return;
    const t = e.target;
    if (!(t instanceof Element)) return;
    if (popoverEl?.contains(t)) return; // a click on the popover itself
    if (t.closest(".card")) return; // a node click re-targets via its own handler
    nodeDetail.close();
  }

  // Anchor-follow (D3): recompute placement when the target/view/transform change,
  // so the popover tracks the node through pan/zoom. Reads rendered rects + the
  // pure placeFor — NO viewBox/transform re-derivation.
  $effect(() => {
    void nodeDetail.target;
    void nodeDetail.view;
    void transform; // follow the node as the canvas pans/zooms
    if (
      nodeDetail.target === null ||
      wrapEl === null ||
      popoverEl === null ||
      activeEl === null
    ) {
      return;
    }
    const wrap = wrapEl.getBoundingClientRect();
    const el = activeEl.getBoundingClientRect();
    const size = popoverEl.getBoundingClientRect();
    pos = placeFor(
      { left: el.left - wrap.left, top: el.top - wrap.top, width: el.width, height: el.height },
      { width: size.width, height: size.height },
      { left: 0, top: 0, width: wrap.width, height: wrap.height },
    );
  });

  // Restore focus to the originating node whenever the popover closes (Esc /
  // outside-click / model change) — a11y. On a re-root the node has unmounted, so
  // the focus is a harmless no-op.
  $effect(() => {
    const open = nodeDetail.target !== null;
    if (!open && wasOpen) activeEl?.focus();
    wasOpen = open;
  });
</script>

<!-- The background click-to-close is a mouse convenience; keyboard users dismiss
     with Esc (owned by the popover) and the nodes own their own keyboard. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="canvas-wrap" bind:this={wrapEl} onclick={onWrapClick}>
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
      <!-- The raised shadow a card lifts to on hover — a larger, softer, lower
           drop that reads as "this card is interactive" (live canvas only; the
           static export's cards don't lift). The wider region avoids clipping
           the bigger blur. -->
      <filter id="card-shadow-hover" x="-40%" y="-40%" width="180%" height="180%">
        <feDropShadow dx="0" dy="3" stdDeviation="5" flood-opacity="0.22" />
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
        <TreeNode {node} portraitUrl={portraitOf(node)} {onreroot} oninspect={onInspect} />
      {/each}
    </g>
  </svg>

  {#if nodeDetail.target !== null}
    <div class="popover-layer" bind:this={popoverEl} style="left: {pos.left}px; top: {pos.top}px">
      <DetailPopover
        view={nodeDetail.view}
        portraitUrl={activePortraitUrl}
        isRoot={nodeDetail.target.id === focalPersonId}
        oncenter={center}
        onopenlibrary={openInLibrary}
        onclose={() => nodeDetail.close()}
      />
    </div>
  {/if}
</div>

<style>
  /* The positioned host: the SVG fills it; the detail popover is absolute over it. */
  .canvas-wrap {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .popover-layer {
    position: absolute;
    z-index: 20;
    /* `left`/`top` come from placeFor (container-relative px), set inline. */
  }
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
