<script lang="ts">
  // SourcesView — the Sources management surface, reached from the
  // DatabaseBar's Library | Tree | Sources switch. Lists the source catalogue and
  // hosts create / edit / delete. Deleting confirms with the exact count of
  // citations it will cascade (fetched via the SourceView), through the in-app
  // ConfirmDialog — no native dialog, no ACL change. All logic is the command's;
  // this owns only the store wiring (reload after each write).

  import * as api from "../lib/api";
  import { asCommandError } from "../lib/errors";
  import { sources } from "../lib/stores/sources.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type { Source } from "../lib/types";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import SourceForm from "./SourceForm.svelte";

  let creating = $state(false);
  let editing = $state<Source | null>(null);
  let confirming = $state<{ source: Source; count: number } | null>(null);

  async function onSaved(): Promise<void> {
    creating = false;
    editing = null;
    await sources.reload();
  }

  function cancel(): void {
    creating = false;
    editing = null;
  }

  // Fetch the citation count first, so the confirm names exactly what cascades.
  async function askDelete(source: Source): Promise<void> {
    try {
      const view = await api.sourceGet(source.id);
      confirming = { source, count: view.citations.length };
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  async function confirmDelete(): Promise<void> {
    const target = confirming?.source;
    confirming = null;
    if (!target) return;
    try {
      await api.sourceDelete(target.id);
      undo.recordDelete(target.title);
      if (editing?.id === target.id) editing = null;
      await sources.reload();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  const deleteBody = $derived(
    confirming === null
      ? ""
      : confirming.count === 0
        ? "This permanently removes the source. It has no citations."
        : `This permanently removes the source and its ${confirming.count} citation${confirming.count === 1 ? "" : "s"}.`,
  );
</script>

<section class="sources" aria-label="Sources">
  {#if creating}
    <div class="panel">
      <SourceForm onsaved={onSaved} oncancel={cancel} />
    </div>
  {:else if editing}
    <div class="panel">
      <SourceForm source={editing} onsaved={onSaved} oncancel={cancel} />
    </div>
  {:else}
    <div class="head">
      <h2 class="title">Sources</h2>
      <button type="button" class="primary" onclick={() => (creating = true)}>
        + New source
      </button>
    </div>

    {#if sources.all.length === 0}
      <p class="hint">No sources yet. Create one to start citing your facts.</p>
    {:else}
      <ul role="list" class="list">
        {#each sources.all as s (s.id)}
          <li>
            <div class="info">
              <span class="src-title">{s.title}</span>
              {#if s.author}<span class="meta">{s.author}</span>{/if}
              {#if s.repository}<span class="meta">· {s.repository}</span>{/if}
            </div>
            <div class="row-actions">
              <button type="button" class="link" onclick={() => (editing = s)}>Edit</button>
              <button type="button" class="link danger" onclick={() => askDelete(s)}>
                Delete
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

{#if confirming}
  <ConfirmDialog
    title="Delete this source?"
    body={deleteBody}
    confirmLabel="Delete"
    danger
    onconfirm={confirmDelete}
    oncancel={() => (confirming = null)}
  />
{/if}

<style>
  .sources {
    height: 100%;
    overflow-y: auto;
    padding: var(--space-6);
    background: var(--color-paper);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: var(--space-4);
  }

  .title {
    font-size: var(--text-xl);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .panel {
    max-width: 40rem;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .list li {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-3) 0;
    border-bottom: 1px solid var(--color-hairline);
  }

  .info {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
  }

  .src-title {
    color: var(--color-ink);
  }

  .meta {
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .row-actions {
    display: flex;
    gap: var(--space-3);
    flex: none;
  }

  .link {
    background: transparent;
    border: none;
    color: var(--color-accent-text);
    font-size: var(--text-sm);
    padding: 0;
  }

  .link.danger {
    color: var(--color-danger);
  }

  button.primary {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    background: var(--color-accent-weak);
    color: var(--color-ink);
  }
</style>
