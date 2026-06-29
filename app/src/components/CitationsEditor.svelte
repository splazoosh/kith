<script lang="ts">
  // CitationsEditor — list / attach / remove the citations on a fact.
  // Self-contained like MediaGallery: it loads `citations_for` for its `subject`
  // and reloads after each write. Attaching picks a source from the shared
  // sources store (so a source created in the Sources view is immediately
  // attachable) plus an optional page / confidence / detail. The frontend never
  // builds provenance — it renders the CitationItem[] the command returns. The
  // GUI authors event citations only; person/family citations arrive via
  // GEDCOM import or the CLI.

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { sources } from "../lib/stores/sources.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type {
    CitationId,
    CitationItem,
    CitationSubject,
    Confidence,
  } from "../lib/types";

  interface Props {
    subject: CitationSubject;
  }
  let { subject }: Props = $props();

  const CONFIDENCES: readonly Confidence[] = [
    "Primary",
    "Secondary",
    "Questionable",
  ];

  let items = $state<CitationItem[]>([]);
  let adding = $state(false);
  let busy = $state(false);
  // A rejected rule shows inline at the form; non-validation errors toast.
  let error = $state<string | null>(null);

  // Attach-form fields (reset whenever the form opens).
  let sourceId = $state<number | null>(null);
  let page = $state("");
  let confidence = $state<Confidence | "">("");
  let detail = $state("");

  // (Re)load whenever the subject changes — keyed on its JSON, since it's an object.
  $effect(() => {
    void JSON.stringify(subject);
    void load();
  });

  async function load(): Promise<void> {
    try {
      items = await api.citationsFor(subject);
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  function openForm(): void {
    sourceId = sources.all[0]?.id ?? null;
    page = "";
    confidence = "";
    detail = "";
    error = null;
    adding = true;
  }

  function blankToNull(s: string): string | null {
    const t = s.trim();
    return t === "" ? null : t;
  }

  async function attach(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    if (sourceId === null) {
      error = "choose a source to cite";
      return;
    }
    busy = true;
    try {
      await api.citationAdd({
        source: sourceId,
        subject,
        page: blankToNull(page),
        detail: blankToNull(detail),
        confidence: confidence === "" ? null : confidence,
      });
      adding = false;
      await load();
    } catch (err) {
      // Validation teaches inline; anything else toasts (the PersonForm pattern).
      if (isCommandError(err) && err.kind === "validation") {
        error = err.message;
      } else {
        toast.pushError(asCommandError(err));
      }
    } finally {
      busy = false;
    }
  }

  async function remove(id: CitationId, label: string): Promise<void> {
    try {
      await api.citationDelete(id);
      undo.recordDelete(`the citation of ${label}`);
      await load();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }
</script>

<div class="citations">
  {#if items.length > 0}
    <ul role="list" class="list">
      {#each items as item (item.citation.id)}
        <li>
          <span class="src">{item.source.title}</span>
          {#if item.citation.page}<span class="meta">{item.citation.page}</span>{/if}
          {#if item.citation.confidence}
            <span class="badge">{item.citation.confidence}</span>
          {/if}
          <button
            type="button"
            class="link danger"
            aria-label="Remove citation"
            onclick={() => remove(item.citation.id, item.source.title)}
          >
            Remove
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="hint">No citations.</p>
  {/if}

  {#if adding}
    {#if sources.all.length === 0}
      <p class="hint">Add a source in the Sources view first.</p>
      <button type="button" class="link" onclick={() => (adding = false)}>Cancel</button>
    {:else}
      <form class="attach" onsubmit={attach}>
        <label class="field">
          <span>Source</span>
          <select bind:value={sourceId}>
            {#each sources.all as s (s.id)}
              <option value={s.id}>{s.title}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span>Page</span>
          <input type="text" bind:value={page} placeholder="e.g. p. 12" />
        </label>
        <label class="field">
          <span>Confidence</span>
          <select bind:value={confidence}>
            <option value="">—</option>
            {#each CONFIDENCES as c (c)}
              <option value={c}>{c}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span>Detail</span>
          <input type="text" bind:value={detail} placeholder="transcription / note" />
        </label>
        {#if error}
          <p class="error" role="alert">{error}</p>
        {/if}
        <div class="actions">
          <button type="button" onclick={() => (adding = false)}>Cancel</button>
          <button type="submit" class="primary" disabled={busy}>Attach</button>
        </div>
      </form>
    {/if}
  {:else}
    <button type="button" class="link" onclick={openForm}>+ Add citation</button>
  {/if}
</div>

<style>
  .citations {
    margin-top: var(--space-2);
    padding-left: var(--space-4);
    border-left: 2px solid var(--color-hairline);
  }

  .list {
    list-style: none;
    margin: 0 0 var(--space-2);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .list li {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
  }

  .src {
    flex: 1;
    font-size: var(--text-sm);
    color: var(--color-ink);
  }

  .meta {
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .badge {
    font-size: var(--text-xs);
    color: var(--color-accent-text);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
    margin: 0 0 var(--space-1);
  }

  .attach {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .error {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--color-danger);
    font-size: var(--text-sm);
  }

  .actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
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
</style>
