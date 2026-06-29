<script lang="ts">
  // SourceForm — create or edit one source, the PersonForm grain.
  // An absent `source` prop means create (an empty draft), a present one edit
  // (seeded from the record). The draft is LOCAL state; success reports
  // through `onsaved`, so the Sources view owns the reload. Only `title` is
  // required; a validation error from the command surfaces inline.

  import { untrack } from "svelte";

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { toast } from "../lib/stores/toast.svelte";
  import type { NewSource, Source } from "../lib/types";

  interface Props {
    source?: Source;
    onsaved: (saved: Source) => void;
    oncancel: () => void;
  }
  let { source, onsaved, oncancel }: Props = $props();

  const seed = untrack(() => source);
  const isEdit = seed !== undefined;
  let title = $state(seed?.title ?? "");
  let author = $state(seed?.author ?? "");
  let publication = $state(seed?.publication ?? "");
  let repository = $state(seed?.repository ?? "");
  let notes = $state(seed?.notes ?? "");

  let error = $state<string | null>(null);
  let busy = $state(false);

  function blankToNull(s: string): string | null {
    const t = s.trim();
    return t === "" ? null : t;
  }

  function draft(): NewSource {
    return {
      title: title.trim(),
      author: blankToNull(author),
      publication: blankToNull(publication),
      repository: blankToNull(repository),
      notes: blankToNull(notes),
    };
  }

  async function submit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    if (title.trim() === "") {
      error = "a source needs a title";
      return;
    }
    busy = true;
    try {
      const saved = seed
        ? await api.sourceUpdate(seed.id, draft())
        : await api.sourceCreate(draft());
      onsaved(saved);
    } catch (err) {
      if (isCommandError(err) && err.kind === "validation") {
        error = err.message;
      } else {
        toast.pushError(asCommandError(err));
      }
    } finally {
      busy = false;
    }
  }
</script>

<form class="source-form" onsubmit={submit}>
  <h2 class="title">{isEdit ? "Edit source" : "New source"}</h2>

  <label class="field">
    <span>Title *</span>
    <input type="text" bind:value={title} />
  </label>
  <label class="field">
    <span>Author</span>
    <input type="text" bind:value={author} />
  </label>
  <label class="field">
    <span>Publication</span>
    <input type="text" bind:value={publication} />
  </label>
  <label class="field">
    <span>Repository</span>
    <input type="text" bind:value={repository} />
  </label>
  <label class="field">
    <span>Notes</span>
    <textarea rows="3" bind:value={notes}></textarea>
  </label>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <div class="actions">
    <button type="button" onclick={oncancel}>Cancel</button>
    <button type="submit" class="primary" disabled={busy}>
      {isEdit ? "Save" : "Create source"}
    </button>
  </div>
</form>

<style>
  .source-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .title {
    font-size: var(--text-lg);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .error {
    margin: 0;
    color: var(--color-danger);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
