<script lang="ts">
  // TreeNode — one positioned node at its OWN model box. Person: a clipped,
  // shadowed card. A SINGLE click (or Enter/Space) inspects — it opens the read-
  // only detail popover; a DOUBLE click re-roots the chart (the fast-navigation
  // path). role=button, tabindex, a focus-visible ring; a quiet per-sex edge +
  // restrained focal ring. Union: a small inert joiner. Names are real <text>
  // (a11y + export parity). No layout math.
  import type { LayoutNode } from "../lib/types";

  interface Props {
    node: LayoutNode;
    /** Resolved asset URL of this person's portrait, or null. */
    portraitUrl?: string | null;
    /** Re-root the chart on this person (double-click / the popover's Center action). */
    onreroot: (personId: number) => void;
    /** Inspect this person (single-click / Enter / Space): open the detail popover.
     *  Receives the person id plus this node's rendered <g> for the popover anchor. */
    oninspect?: (personId: number, groupEl: SVGGElement) => void;
  }
  let { node, portraitUrl = null, onreroot, oninspect }: Props = $props();

  // The single-vs-double-click disambiguation window (~a platform double-click).
  // A single click waits this long so a double-click (two clicks first) can cancel
  // it and re-root instead of also flashing the popover.
  const DBL_MS = 220;

  // Portrait avatar geometry, mirroring the HTML exporter's metrics so the live
  // canvas and the exported file match (PORTRAIT_D / PORTRAIT_INSET). The card
  // box itself never changes — the avatar overlays its left edge.
  const PORTRAIT_D = 48;
  const PORTRAIT_INSET = 12;
  const hasPortrait = $derived(portraitUrl != null);
  // Name/lifespan slide right of the avatar when a portrait is present.
  const textX = $derived(hasPortrait ? PORTRAIT_INSET + PORTRAIT_D + PORTRAIT_INSET : 14);
  const portraitCy = $derived(node.height / 2);

  // Unique within the document (node ids are unique across the model), so the
  // per-card clips never collide with another card's.
  const clipId = $derived(`card-clip-${node.id}`);
  const portraitClipId = $derived(`portrait-clip-${node.id}`);

  const label = (c: NonNullable<LayoutNode["content"]>): string =>
    c.lifespan ? `${c.display_name} (${c.lifespan})` : c.display_name;

  // The rendered <g>, handed to `oninspect` so the host can anchor the popover.
  let groupEl = $state<SVGGElement | null>(null);
  let clickTimer: ReturnType<typeof setTimeout> | undefined;

  /** This node's person id, or null for a union (which never reaches these handlers). */
  function personId(): number | null {
    return node.kind === "Person" && "Person" in node.entity ? node.entity.Person : null;
  }

  function inspect(): void {
    const id = personId();
    if (id !== null && groupEl !== null) oninspect?.(id, groupEl);
  }
  function reroot(): void {
    const id = personId();
    if (id !== null) onreroot(id);
  }

  // Single click inspects — but debounced so a double-click (two clicks first)
  // cancels it and re-roots instead of also flashing the popover.
  function onclick(): void {
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => {
      clickTimer = undefined;
      inspect();
    }, DBL_MS);
  }
  function ondblclick(): void {
    clearTimeout(clickTimer);
    clickTimer = undefined;
    reroot();
  }
  function onkeydown(e: KeyboardEvent): void {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      inspect(); // keyboard has no "double" — re-root is the popover's Center action
    }
  }

  // A late timer after the nodes were replaced (a re-root) would inspect a stale
  // id — clear it on unmount.
  $effect(() => () => clearTimeout(clickTimer));
</script>

{#if node.kind === "Person" && node.content}
  <g
    bind:this={groupEl}
    class="card"
    class:focal={node.focal}
    transform={`translate(${node.x} ${node.y})`}
    role="button"
    tabindex="0"
    aria-label={`Show details for ${label(node.content)}`}
    {onclick}
    {ondblclick}
    {onkeydown}
  >
    <title>{label(node.content)}</title>
    <clipPath id={clipId}>
      <rect width={node.width} height={node.height} rx="8" />
    </clipPath>
    <rect class="bg" width={node.width} height={node.height} rx="8" filter="url(#card-shadow)" />
    <rect class="sex sex-{node.content.sex.toLowerCase()}" width="4" height={node.height} />
    <g clip-path={`url(#${clipId})`}>
      {#if hasPortrait}
        <clipPath id={portraitClipId}>
          <circle cx={PORTRAIT_INSET + PORTRAIT_D / 2} cy={portraitCy} r={PORTRAIT_D / 2} />
        </clipPath>
        <image
          href={portraitUrl}
          x={PORTRAIT_INSET}
          y={portraitCy - PORTRAIT_D / 2}
          width={PORTRAIT_D}
          height={PORTRAIT_D}
          preserveAspectRatio="xMidYMid slice"
          clip-path={`url(#${portraitClipId})`}
        />
        <circle
          class="portrait"
          cx={PORTRAIT_INSET + PORTRAIT_D / 2}
          cy={portraitCy}
          r={PORTRAIT_D / 2}
        />
      {/if}
      <text class="name" x={textX} y="30">{node.content.display_name}</text>
      {#if node.content.lifespan}
        <text class="lifespan" x={textX} y="50">{node.content.lifespan}</text>
      {/if}
    </g>
    <!-- The focus ring is an SVG rect (outline on <g> is inconsistent). -->
    <rect class="focus-ring" width={node.width} height={node.height} rx="8" />
  </g>
{:else}
  <!-- Union joiner: small, quiet, non-interactive. -->
  <circle
    class="union"
    cx={node.x + node.width / 2}
    cy={node.y + node.height / 2}
    r={node.width / 2}
  />
{/if}

<style>
  .card {
    cursor: pointer;
  }
  .bg {
    fill: var(--color-surface);
    stroke: var(--color-hairline);
    stroke-width: 1;
    /* Smooth the hover cue below (neutralized under prefers-reduced-motion by
       the global reset). The filter swap itself is discrete — not listed. */
    transition:
      stroke var(--motion-fast),
      stroke-width var(--motion-fast),
      fill var(--motion-fast);
  }
  /* Focal: a restrained ring distinguishable WITHOUT color alone (weight). */
  .card.focal .bg {
    stroke: var(--tree-focal);
    stroke-width: 2.5;
  }
  /* Signal that a card is clickable: on hover it lifts (a raised shadow), takes
     the accent edge that the app's buttons use on hover, and warms a touch —
     "click to explore this person". A focal card keeps its heavier emphasis
     ring (the :not(.focal) guard), just gaining the lift + warmth. */
  .card:hover .bg {
    fill: color-mix(in srgb, var(--color-surface) 92%, var(--color-accent));
    filter: url(#card-shadow-hover);
  }
  .card:not(.focal):hover .bg {
    stroke: var(--color-accent);
    stroke-width: 1.5;
  }
  .card:hover .name {
    fill: var(--color-accent-text);
  }
  .name {
    font-family: var(--font-serif);
    font-size: 1rem;
    fill: var(--color-ink);
    transition: fill var(--motion-fast);
  }
  .lifespan {
    font-family: var(--font-sans);
    font-size: 0.8125rem;
    fill: var(--color-ink-soft);
  }
  /* A hairline ring around the circular portrait, matching the export. */
  .portrait {
    fill: none;
    stroke: var(--color-hairline);
    stroke-width: 1;
  }
  /* Quiet per-sex edge (an accent, not a color block). */
  .sex-male {
    fill: var(--tree-sex-male);
  }
  .sex-female {
    fill: var(--tree-sex-female);
  }
  .sex-other,
  .sex-unknown {
    fill: var(--tree-sex-unknown);
  }
  .union {
    fill: var(--color-ink-soft);
    opacity: 0.6;
  }
  /* Suppress the inconsistent <g> outline; show our SVG ring on :focus-visible. */
  .card:focus {
    outline: none;
  }
  .focus-ring {
    fill: none;
    stroke: none;
    pointer-events: none;
  }
  .card:focus-visible .focus-ring {
    stroke: var(--color-accent);
    stroke-width: 3;
  }
</style>
