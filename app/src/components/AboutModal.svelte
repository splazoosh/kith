<script lang="ts">
  // AboutModal — the About/Help modal. Gives the app an identity: product name +
  // version (from the `about_info` command — the one source of truth), the MIT
  // license, the offline/no-account/no-telemetry promise, a link to the
  // keyboard-shortcut reference, and (when set) the repository URL as SELECTABLE
  // TEXT (not a clickable external link — that would need tauri-plugin-opener + a
  // new ACL entry; selectable text keeps least-privilege). The backdrop / Esc /
  // Tab-trap / focus / modal.open() wiring mirrors ConfirmDialog / ExportDialog.

  import { onMount, tick } from "svelte";

  import { aboutInfo } from "../lib/api";
  import { asCommandError } from "../lib/errors";
  import { shortcutsHelp } from "../lib/shortcuts.svelte";
  import { modal } from "../lib/stores/modal.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  import type { AboutInfo } from "../lib/types";

  interface Props {
    onclose: () => void;
  }
  let { onclose }: Props = $props();

  let info = $state<AboutInfo | null>(null);

  // Fetch the identity once mounted (the version is read from the binary).
  onMount(async () => {
    try {
      info = await aboutInfo();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  });

  let dialog = $state<HTMLDivElement | null>(null);
  let firstControl = $state<HTMLButtonElement | null>(null);

  // Focus the first control once mounted (the modal owns focus while open).
  $effect(() => {
    void tick().then(() => firstControl?.focus());
  });

  // Hold the keyboard while open, so the shortcut registry stands down.
  $effect(() => modal.open());

  /** Hand off to the shortcut reference: close About, then open ShortcutsHelp
   *  (one modal at a time — the modal guard expects a single owner). */
  function openShortcuts(): void {
    onclose();
    shortcutsHelp.show();
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return;
    }
    if (e.key !== "Tab" || dialog === null) return;
    // Trap Tab within the dialog's focusable controls.
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

<!-- The backdrop: a click outside the dialog closes. -->
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
    aria-labelledby="about-title"
    bind:this={dialog}
  >
    <h2 id="about-title">{info?.name ?? "Kith"}</h2>
    <p class="version">Version {info?.version ?? "—"}</p>

    <p class="tagline">A local-first family-tree desktop app.</p>

    <p class="privacy">
      Kith works entirely offline. Your family tree stays on this machine — no
      account, no server, no telemetry.
    </p>

    <dl class="meta">
      <div class="row">
        <dt>License</dt>
        <dd>{info?.license ?? "MIT"}</dd>
      </div>
      {#if info?.repository}
        <div class="row">
          <dt>Repository</dt>
          <!-- Selectable text, NOT a clickable external link (keeps the ACL
               least-privilege — no opener/shell grant). Copy it to open it. -->
          <dd class="repo">{info.repository}</dd>
        </div>
      {/if}
    </dl>

    <div class="actions">
      <button type="button" bind:this={firstControl} onclick={openShortcuts}>
        Keyboard shortcuts
      </button>
      <button type="button" class="confirm" onclick={onclose}>Close</button>
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
    font-size: var(--text-xl);
    color: var(--color-ink);
    margin-bottom: var(--space-1);
  }

  .version {
    margin: 0 0 var(--space-4);
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .tagline {
    margin: 0 0 var(--space-3);
    color: var(--color-ink);
  }

  .privacy {
    margin: 0 0 var(--space-5);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .meta {
    margin: 0 0 var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
  }

  dt {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
    flex: none;
  }

  dd {
    margin: 0;
    color: var(--color-ink);
    text-align: right;
  }

  .repo {
    font-family: var(--font-mono, monospace);
    font-size: var(--text-xs);
    user-select: text;
    word-break: break-all;
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

  button:hover {
    border-color: var(--color-accent);
  }

  .confirm {
    background: var(--color-accent-weak);
    border-color: var(--color-accent);
  }
</style>
