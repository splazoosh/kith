<script lang="ts">
  // ExportDialog — the static-HTML export options modal. A pure
  // options collector: it gathers mode / generations / theme / include-living
  // (root fixed to the current chart's focal, shown read-only) and hands them to
  // exportChart, which owns the save dialog + IPC + toast. NO saveDialog/
  // api/toast calls live here. The focus-trap + Esc wiring mirrors ConfirmDialog.

  import { tick } from "svelte";

  import { exportChart } from "../lib/exportActions";
  import { chart } from "../lib/stores/chart.svelte";
  import { modal } from "../lib/stores/modal.svelte";
  import type { ChartMode, Theme } from "../lib/types";

  interface Props {
    /** The focal person id (the chart root) and a display name for the read-only root field. */
    rootId: number;
    focalName: string;
    onclose: () => void;
  }
  let { rootId, focalName, onclose }: Props = $props();

  const MODES: ChartMode[] = ["Ancestors", "Descendants", "Hourglass", "Network"];
  const MIN_GEN = 1;
  const MAX_GEN = 8;

  // Seed from the current chart; the user can override mode/generations/theme/include-living.
  let mode = $state<ChartMode>(chart.mode);
  let generations = $state(chart.generations);
  let theme = $state<Theme>("Light"); // the export document's palette (Theme.default())
  let includeLiving = $state(false); // redact by default — the checkbox is the only opt-out
  let includePortraits = $state(false); // base64-embed portraits (off by default)
  let busy = $state(false);

  // Network lays out the whole connected component; depth does not apply.
  const depthDisabled = $derived(mode === "Network");

  let dialog = $state<HTMLDivElement | null>(null);
  let firstControl = $state<HTMLSelectElement | null>(null);

  // Focus the first control once mounted (the modal owns focus while open).
  $effect(() => {
    void tick().then(() => firstControl?.focus());
  });

  // Hold the keyboard while open, so the shortcut registry stands down.
  $effect(() => modal.open());

  async function confirm(): Promise<void> {
    busy = true;
    await exportChart({
      root: rootId,
      mode,
      generations,
      theme,
      includeLiving,
      includePortraits,
      defaultName: `${focalName} ${mode}`,
    });
    busy = false;
    onclose(); // close whether a file was written or the save was cancelled
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key !== "Tab" || dialog === null) return;
    // Trap Tab within the dialog's focusable controls.
    const focusable = dialog.querySelectorAll<HTMLElement>(
      "button, select, input",
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (e.shiftKey && active === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- The backdrop: a click outside the dialog closes (a no-op cancel). -->
<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="export-title"
    bind:this={dialog}
  >
    <h2 id="export-title">Export chart</h2>

    <div class="field">
      <span class="label">Root</span>
      <output class="root">{focalName}</output>
    </div>

    <label class="field">
      <span class="label">Mode</span>
      <select bind:value={mode} bind:this={firstControl}>
        {#each MODES as m (m)}
          <option value={m}>{m}</option>
        {/each}
      </select>
    </label>

    <label class="field">
      <span class="label">Depth <output>{depthDisabled ? "—" : generations}</output></span>
      <input
        type="range"
        min={MIN_GEN}
        max={MAX_GEN}
        bind:value={generations}
        disabled={depthDisabled}
        aria-label="Generation depth"
        title={depthDisabled ? "Network exports the whole connected graph" : undefined}
      />
    </label>

    <label class="field">
      <span class="label">Theme</span>
      <select bind:value={theme}>
        <option value="Light">Light</option>
        <option value="Dark">Dark</option>
      </select>
    </label>

    <label class="field checkbox">
      <input type="checkbox" bind:checked={includeLiving} />
      <span>Include living individuals' details</span>
    </label>

    <label class="field checkbox">
      <input type="checkbox" bind:checked={includePortraits} />
      <span>Include portraits</span>
    </label>

    <div class="actions">
      <button type="button" onclick={onclose} disabled={busy}>Cancel</button>
      <button type="button" class="confirm" onclick={confirm} disabled={busy}>
        Export…
      </button>
    </div>
  </div>
</div>

<style>
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
    margin-bottom: var(--space-5);
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-4);
  }

  .label {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .root {
    color: var(--color-ink);
    font-weight: 500;
    text-align: right;
  }

  .field.checkbox {
    justify-content: flex-start;
    gap: var(--space-2);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  select,
  input[type="range"] {
    min-width: 10rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    margin-top: var(--space-6);
  }

  button {
    padding: var(--space-2) var(--space-4);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
  }

  button:hover:not(:disabled) {
    border-color: var(--color-accent);
  }

  button:disabled {
    opacity: 0.5;
  }

  .confirm {
    background: var(--color-accent-weak);
    border-color: var(--color-accent);
  }
</style>
