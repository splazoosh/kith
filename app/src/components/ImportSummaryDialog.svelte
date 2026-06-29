<script lang="ts">
  // ImportSummaryDialog — an acknowledge-only modal shown after a GEDCOM import.
  // Mirrors ConfirmDialog's modal/a11y contract (backdrop,
  // role="alertdialog", an $effect focus, Esc/backdrop-click close) but carries a
  // single "Done" button: it reports the counts the engine wrote plus, when
  // anything was skipped, the deferred-records line. No GEDCOM/IO
  // logic — it only renders the ImportSummary the action returned.

  import { tick } from "svelte";

  import { modal } from "../lib/stores/modal.svelte";
  import type { ImportSummary } from "../lib/types";

  interface Props {
    summary: ImportSummary;
    onclose: () => void;
  }
  let { summary, onclose }: Props = $props();

  let doneBtn = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    void tick().then(() => doneBtn?.focus());
  });

  // Hold the keyboard while open, so the shortcut registry stands down.
  $effect(() => modal.open());

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  // The deferred (skipped) records, as "SOUR×2, OBJE×1" — empty when nothing was skipped.
  const skipped = $derived(
    Object.entries(summary.skipped_tags)
      .map(([tag, n]) => `${tag}×${n}`)
      .join(", "),
  );
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div
    class="dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="import-title"
  >
    <h2 id="import-title">Import complete</h2>
    <ul>
      <li>{summary.individuals} individuals</li>
      <li>{summary.families} families</li>
      <li>{summary.events} events</li>
      <li>{summary.names} alternate names</li>
      <li>{summary.places} places</li>
    </ul>
    {#if skipped}
      <p class="skipped">Skipped unsupported records: {skipped}</p>
    {/if}
    <div class="actions">
      <button type="button" bind:this={doneBtn} onclick={onclose}>Done</button>
    </div>
  </div>
</div>

<style>
  /* Mirrors ConfirmDialog.svelte: a dimmed fixed backdrop centring a token-styled
     surface, and an actions row with a single button. */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    background: rgba(0, 0, 0, 0.45);
  }

  .dialog {
    width: min(28rem, 100%);
    padding: var(--space-6);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-2);
  }

  h2 {
    font-size: var(--text-lg);
    margin-bottom: var(--space-4);
  }

  ul {
    margin: 0 0 var(--space-4);
    padding-left: var(--space-6);
    color: var(--color-ink);
  }

  li {
    margin-bottom: var(--space-1);
  }

  .skipped {
    margin: 0 0 var(--space-6);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }

  button {
    padding: var(--space-2) var(--space-4);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
  }

  button:hover {
    border-color: var(--color-accent);
  }
</style>
