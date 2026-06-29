<script lang="ts">
  // NamesEditor — list / add / remove an individual's alternate names. Self-
  // contained: it loads `name_list` for its `individualId` and reloads after
  // each write, so it needs no shared store. Add appends a `NewName` (its
  // `sort_order` is the current count); remove is reversible, so it just shows a
  // toast notice (no confirm modal).

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { displayName } from "../lib/format";
  import { toast } from "../lib/stores/toast.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type { Name, NameKind } from "../lib/types";

  interface Props {
    individualId: number;
  }
  let { individualId }: Props = $props();

  const NAME_KINDS: readonly NameKind[] = ["Birth", "Married", "Aka", "Religious"];

  let names = $state<Name[]>([]);
  let adding = $state(false);
  let kind = $state<NameKind>("Aka");
  let given = $state("");
  let surname = $state("");
  let prefix = $state("");
  let suffix = $state("");
  let busy = $state(false);
  // A rejected rule shows inline at the form; non-validation errors toast (the
  // PersonForm pattern).
  let error = $state<string | null>(null);

  // (Re)load whenever the person changes.
  $effect(() => {
    void load(individualId);
  });

  async function load(id: number): Promise<void> {
    try {
      names = await api.nameList(id);
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  function reset(): void {
    adding = false;
    kind = "Aka";
    given = "";
    surname = "";
    prefix = "";
    suffix = "";
    error = null;
  }

  function blankToNull(s: string): string | null {
    const t = s.trim();
    return t === "" ? null : t;
  }

  async function add(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    // Cheap client-side guard: an alternate name needs at least a given or surname.
    if (given.trim() === "" && surname.trim() === "") {
      error = "an alternate name needs a given name or a surname";
      return;
    }
    busy = true;
    try {
      await api.nameAdd({
        individual_id: individualId,
        kind,
        given_name: blankToNull(given),
        surname: blankToNull(surname),
        name_prefix: blankToNull(prefix),
        name_suffix: blankToNull(suffix),
        sort_order: names.length,
      });
      reset();
      await load(individualId);
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

  async function remove(id: number, label: string): Promise<void> {
    try {
      await api.nameRemove(id);
      undo.recordDelete(`the name "${label}"`);
      await load(individualId);
    } catch (err) {
      toast.pushError(asCommandError(err));
    }
  }
</script>

<section class="names">
  <div class="head">
    <h3>Alternate names</h3>
    {#if !adding}
      <button type="button" class="link" onclick={() => (adding = true)}>
        + Add name
      </button>
    {/if}
  </div>

  {#if names.length > 0}
    <ul role="list">
      {#each names as n (n.id)}
        <li>
          <span class="name">{displayName(n)}</span>
          <span class="kind">{n.kind}</span>
          <button
            type="button"
            class="link danger"
            aria-label="Remove name"
            onclick={() => remove(n.id, displayName(n))}
          >
            Remove
          </button>
        </li>
      {/each}
    </ul>
  {:else if !adding}
    <p class="hint">No alternate names.</p>
  {/if}

  {#if adding}
    <form class="add" onsubmit={add}>
      <div class="row">
        <label class="field">
          <span>Kind</span>
          <select bind:value={kind}>
            {#each NAME_KINDS as k (k)}
              <option value={k}>{k}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span>Given</span>
          <input type="text" bind:value={given} />
        </label>
        <label class="field">
          <span>Surname</span>
          <input type="text" bind:value={surname} />
        </label>
      </div>
      <div class="row">
        <label class="field">
          <span>Prefix</span>
          <input type="text" bind:value={prefix} />
        </label>
        <label class="field">
          <span>Suffix</span>
          <input type="text" bind:value={suffix} />
        </label>
      </div>
      {#if error}
        <p class="error" role="alert">{error}</p>
      {/if}
      <div class="actions">
        <button type="button" onclick={reset}>Cancel</button>
        <button type="submit" class="primary" disabled={busy}>Add</button>
      </div>
    </form>
  {/if}
</section>

<style>
  .names {
    margin-top: var(--space-6);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  h3 {
    font-size: var(--text-md);
    color: var(--color-ink-soft);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  li {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--color-hairline);
  }

  li .name {
    flex: 1;
  }

  .kind {
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .add {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-top: var(--space-3);
    padding: var(--space-4);
    background: var(--color-surface-2);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .row {
    display: flex;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    flex: 1;
    min-width: 8rem;
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
