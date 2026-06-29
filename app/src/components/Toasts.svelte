<script lang="ts">
  // Toasts — renders the toast channel. Errors are sticky (a close button);
  // notices auto-dismiss via the store's TTL. The single visual sink for every
  // CommandError a store catch funnels in.

  import type { Toast } from "../lib/stores/toast.svelte";
  import { toast } from "../lib/stores/toast.svelte";

  function tagLabel(kind: Toast["kind"]): string {
    switch (kind) {
      case "not_found":
        return "Not found";
      case "validation":
        return "Invalid";
      case "io":
        return "File";
      case "database":
        return "Database";
      case "notice":
        return "Notice";
      default:
        return "Error";
    }
  }
</script>

{#if toast.items.length > 0}
  <div class="toasts" aria-live="polite">
    {#each toast.items as t (t.id)}
      <div
        class="toast"
        class:notice={t.kind === "notice"}
        role={t.kind === "notice" ? "status" : "alert"}
      >
        <div class="body">
          <span class="tag">{tagLabel(t.kind)}</span>
          <span class="msg">{t.message}</span>
        </div>
        {#if t.action}
          <button class="action" type="button" onclick={() => t.action?.run()}>
            {t.action.label}
          </button>
        {/if}
        {#if t.sticky}
          <button
            class="close"
            type="button"
            aria-label="Dismiss"
            onclick={() => toast.dismiss(t.id)}
          >
            ×
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .toasts {
    position: fixed;
    right: var(--space-4);
    bottom: var(--space-4);
    z-index: 50;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: min(28rem, calc(100vw - 2 * var(--space-4)));
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--color-danger-weak);
    color: var(--color-ink);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-2);
    font-size: var(--text-sm);
  }

  .toast.notice {
    background: var(--color-surface);
    border-color: var(--color-hairline);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .tag {
    font-size: var(--text-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-danger);
    font-weight: 600;
  }

  .notice .tag {
    color: var(--color-ink-soft);
  }

  .msg {
    overflow-wrap: anywhere;
  }

  .action {
    flex: none;
    margin-left: auto;
    align-self: center;
    background: transparent;
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    color: var(--color-accent-text);
    font-size: var(--text-sm);
    font-weight: 600;
    padding: var(--space-1) var(--space-3);
  }

  .action:hover {
    background: var(--color-accent-weak);
  }

  .close {
    flex: none;
    margin-left: auto;
    background: transparent;
    border: none;
    color: var(--color-ink-soft);
    font-size: var(--text-lg);
    line-height: 1;
    padding: 0 var(--space-1);
  }

  .close:hover {
    color: var(--color-ink);
  }
</style>
