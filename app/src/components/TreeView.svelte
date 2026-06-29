<script lang="ts">
  // TreeView — the chart container: the controls bar (mode / depth / fit), and
  // the no-root / loading / error / canvas states. It owns no geometry — it
  // drives the chart store and hands the model to TreeCanvas. The view switch
  // lives in the DatabaseBar; the theme toggle stays there too.
  import { treeFit } from "../lib/shortcuts.svelte";
  import { chart } from "../lib/stores/chart.svelte";
  import { media } from "../lib/stores/media.svelte";
  import type { ChartMode, LayoutModel } from "../lib/types";
  import EmptyState from "./EmptyState.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import TreeCanvas from "./TreeCanvas.svelte";

  const MODES: ChartMode[] = ["Ancestors", "Descendants", "Hourglass", "Network"];
  const MIN_GEN = 1;
  const MAX_GEN = 8;

  // Network lays out the whole connected component, so generation depth does not
  // apply — the slider is disabled (and the value is ignored server-side).
  const depthDisabled = $derived(chart.mode === "Network");

  let canvasFit: (() => void) | null = null;
  const onready = (h: { fit: () => void }): void => {
    canvasFit = h.fit;
    treeFit.set(h.fit); // expose fit to the global `F` shortcut
  };

  // Drop the global fit handle when the Tree view unmounts (a stale handle would
  // fit a chart that is no longer on screen).
  $effect(() => () => treeFit.set(null));

  let exporting = $state(false);

  // Resolve portraits for the current model in one batched IPC. Re-runs on
  // re-root / mode / depth — the canvas draws cards immediately and fills in
  // portraits when the batch resolves (never blocking the render).
  $effect(() => {
    void chart.model;
    void media.resolvePortraits(chart.model);
  });

  /** The focal node's display name, for the export dialog + the default filename. */
  function focalDisplayName(model: LayoutModel | null): string | null {
    return model?.nodes.find((n) => n.focal)?.content?.display_name ?? null;
  }
</script>

<section class="tree">
  <header class="bar" aria-label="Chart controls">
    <div class="modes" role="group" aria-label="Chart mode">
      {#each MODES as m (m)}
        <button
          type="button"
          class:active={chart.mode === m}
          aria-pressed={chart.mode === m}
          onclick={() => chart.setMode(m)}
        >
          {m}
        </button>
      {/each}
    </div>

    <label class="depth" class:disabled={depthDisabled}>
      Depth <output>{depthDisabled ? "—" : chart.generations}</output>
      <input
        type="range"
        min={MIN_GEN}
        max={MAX_GEN}
        value={chart.generations}
        oninput={(e) => chart.setGenerations(e.currentTarget.valueAsNumber)}
        disabled={depthDisabled}
        aria-label="Generation depth"
        title={depthDisabled ? "Network shows the whole connected graph" : undefined}
      />
    </label>

    <button type="button" onclick={() => canvasFit?.()} disabled={!chart.model}>
      Fit
    </button>

    {#if chart.rootId !== null}
      <button type="button" onclick={() => (exporting = true)}>Export…</button>
    {/if}
  </header>

  {#if exporting && chart.rootId !== null}
    <ExportDialog
      rootId={chart.rootId}
      focalName={focalDisplayName(chart.model) ?? "this chart"}
      onclose={() => (exporting = false)}
    />
  {/if}

  <div class="stage">
    {#if chart.rootId === null}
      <EmptyState mode="tree-no-root" />
    {:else if chart.error && !chart.model}
      <p class="status error">{chart.error}</p>
    {:else if chart.model}
      <TreeCanvas
        model={chart.model}
        portraitUrls={media.portraitUrls}
        onreroot={(id) => chart.reroot(id)}
        {onready}
      />
      {#if chart.loading}
        <div class="veil" aria-hidden="true"></div>
      {/if}
    {:else if chart.loading}
      <p class="status">Computing layout…</p>
    {/if}
  </div>
</section>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3);
    border-bottom: 1px solid var(--color-hairline);
    background: var(--color-surface);
  }
  .modes {
    display: flex;
    gap: var(--space-1);
  }
  .modes button {
    padding: var(--space-1) var(--space-3);
    background: transparent;
    border: 1px solid var(--color-hairline);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }
  .modes button.active {
    color: var(--color-ink);
    border-color: var(--color-accent);
    background: var(--color-accent-weak);
  }
  .depth {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }
  .depth input {
    width: 8rem;
  }
  /* Network ignores depth — fade the control to signal it is inert. */
  .depth.disabled {
    opacity: 0.5;
  }
  .stage {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .status {
    padding: var(--space-6);
    color: var(--color-ink-soft);
    text-align: center;
  }
  .status.error {
    color: var(--color-danger);
  }
  /* A quiet veil over the prior chart while a re-fetch is in flight. */
  .veil {
    position: absolute;
    inset: 0;
    background: var(--color-paper);
    opacity: 0.4;
    pointer-events: none;
  }
</style>
