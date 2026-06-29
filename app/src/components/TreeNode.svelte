<script lang="ts">
  // TreeNode — one positioned node at its OWN model box. Person: a clipped,
  // shadowed card that re-roots on click/Enter — role=button, tabindex,
  // a focus-visible ring; a quiet per-sex edge + restrained focal ring.
  // Union: a small inert joiner. Names are real <text> (a11y + export
  // parity). No layout math.
  import type { LayoutNode } from "../lib/types";

  interface Props {
    node: LayoutNode;
    /** Resolved asset URL of this person's portrait, or null. */
    portraitUrl?: string | null;
    onreroot: (personId: number) => void;
  }
  let { node, portraitUrl = null, onreroot }: Props = $props();

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

  // Person nodes carry { Person: id }; unions are inert and never reach this path.
  function activate(): void {
    if (node.kind === "Person" && "Person" in node.entity) onreroot(node.entity.Person);
  }
  function onkeydown(e: KeyboardEvent): void {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      activate();
    }
  }
</script>

{#if node.kind === "Person" && node.content}
  <g
    class="card"
    class:focal={node.focal}
    transform={`translate(${node.x} ${node.y})`}
    role="button"
    tabindex="0"
    aria-label={`Re-root on ${label(node.content)}`}
    onclick={activate}
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
  }
  /* Focal: a restrained ring distinguishable WITHOUT color alone (weight). */
  .card.focal .bg {
    stroke: var(--tree-focal);
    stroke-width: 2.5;
  }
  .name {
    font-family: var(--font-serif);
    font-size: 1rem;
    fill: var(--color-ink);
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
