<script lang="ts">
  // ShortcutsHelp — the `?` keyboard-shortcut reference overlay. A
  // read-only modal fed by the shortcut registry's label/group metadata; the
  // focus-trap + Esc + backdrop wiring mirrors ExportDialog. The About modal links
  // here. Mounted by App only while open, so it registers as a modal for its
  // lifetime (the registry stands down while it is up).

  import { groupedShortcuts, shortcutsHelp } from "../lib/shortcuts.svelte";
  import { modal } from "../lib/stores/modal.svelte";

  // Hold the keyboard while open (the registry checks modal.isOpen).
  $effect(() => modal.open());

  const isMac =
    typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  const MOD = isMac ? "⌘" : "Ctrl";

  /** Render a registry `combo` as display keys ("mod+k" → "Ctrl + K"). */
  function keys(combo: string): string[] {
    if (combo === "?") return ["?"];
    return combo.split("+").map((k) => (k === "mod" ? MOD : k.toUpperCase()));
  }

  const groups = groupedShortcuts();

  let dialog = $state<HTMLDivElement | null>(null);

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      shortcutsHelp.close();
      return;
    }
    if (e.key !== "Tab" || dialog === null) return;
    const focusable = dialog.querySelectorAll<HTMLElement>("button");
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

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) shortcutsHelp.close();
  }}
>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="shortcuts-title"
    bind:this={dialog}
  >
    <h2 id="shortcuts-title">Keyboard shortcuts</h2>

    {#each groups as group (group.name)}
      <h3>{group.name}</h3>
      <dl>
        {#each group.items as s (s.combo)}
          <div class="binding">
            <dt>
              {#each keys(s.combo) as k (k)}
                <kbd>{k}</kbd>
              {/each}
            </dt>
            <dd>{s.label}</dd>
          </div>
        {/each}
      </dl>
    {/each}

    <h3>Dialogs</h3>
    <dl>
      <div class="binding">
        <dt><kbd>Esc</kbd></dt>
        <dd>Close the open dialog or palette</dd>
      </div>
    </dl>

    <div class="actions">
      <button type="button" class="confirm" onclick={() => shortcutsHelp.close()}>
        Close
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
    max-height: 85vh;
    overflow-y: auto;
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

  h3 {
    font-size: var(--text-sm);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-ink-soft);
    margin: var(--space-4) 0 var(--space-2);
  }

  dl {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .binding {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
  }

  dt {
    display: flex;
    gap: var(--space-1);
    flex: none;
  }

  dd {
    margin: 0;
    color: var(--color-ink);
    text-align: right;
  }

  kbd {
    font-family: var(--font-mono, monospace);
    font-size: var(--text-xs);
    padding: 2px var(--space-2);
    background: var(--color-surface-2);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-sm);
    color: var(--color-ink);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: var(--space-6);
  }

  .confirm {
    padding: var(--space-2) var(--space-4);
    background: var(--color-accent-weak);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    color: var(--color-ink);
  }

  .confirm:hover {
    border-color: var(--color-accent);
  }
</style>
