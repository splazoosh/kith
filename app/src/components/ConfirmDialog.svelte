<script lang="ts">
  // ConfirmDialog — a small in-app, focus-trapped modal gating the cascading
  // deletes. Deliberately NOT the native dialog plugin's ask/confirm:
  // those would need a new ACL grant, and a local-first no-network app keeps the
  // minimum surface. Esc and a backdrop click cancel; the confirm button
  // carries the danger accent. Pure DOM over the existing window.

  import { tick } from "svelte";

  import { modal } from "../lib/stores/modal.svelte";

  interface Props {
    title: string;
    body: string;
    confirmLabel: string;
    danger?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  }
  let {
    title,
    body,
    confirmLabel,
    danger = false,
    onconfirm,
    oncancel,
  }: Props = $props();

  let dialog = $state<HTMLDivElement | null>(null);
  let confirmBtn = $state<HTMLButtonElement | null>(null);

  // Focus the confirm button once mounted (the modal owns focus while open).
  $effect(() => {
    void tick().then(() => confirmBtn?.focus());
  });

  // Hold the keyboard while open, so the shortcut registry stands down.
  $effect(() => modal.open());

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      oncancel();
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

<!-- The backdrop: a click outside the dialog cancels. -->
<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) oncancel();
  }}
>
  <div
    class="dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="confirm-title"
    aria-describedby="confirm-body"
    bind:this={dialog}
  >
    <h2 id="confirm-title">{title}</h2>
    <p id="confirm-body">{body}</p>
    <div class="actions">
      <button type="button" onclick={oncancel}>Cancel</button>
      <button
        type="button"
        class="confirm"
        class:danger
        bind:this={confirmBtn}
        onclick={onconfirm}
      >
        {confirmLabel}
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
    margin-bottom: var(--space-3);
  }

  p {
    margin: 0 0 var(--space-6);
    color: var(--color-ink-soft);
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

  .confirm.danger {
    background: var(--color-danger);
    border-color: var(--color-danger);
    color: var(--color-surface);
  }
</style>
