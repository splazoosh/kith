<script lang="ts">
  // PersonPicker — search-and-pick an existing individual, backing the
  // family partner/child slots. A debounced <input> → the `search` command;
  // an empty query falls back to the already-loaded `library.allPeople` so a
  // pick is possible without typing. No new store and no layout/graph logic —
  // it reads the library and calls `search` through the one typed client.

  import * as api from "../lib/api";
  import { asCommandError } from "../lib/errors";
  import { displayName } from "../lib/format";
  import { library } from "../lib/stores/library.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  import type { Individual } from "../lib/types";

  interface Props {
    onpick: (person: Individual) => void;
    /** Ids already chosen (a partner slot, existing children) — hidden here. */
    exclude?: number[];
    placeholder?: string;
  }
  let { onpick, exclude = [], placeholder = "Search people…" }: Props = $props();

  const DEBOUNCE_MS = 150;
  const LIMIT = 20;

  let query = $state("");
  let found = $state<Individual[] | null>(null); // null ⇒ no search run yet
  let timer: ReturnType<typeof setTimeout> | undefined;

  const excluded = $derived(new Set(exclude));
  // The empty-query fallback (the loaded list) or the server search result.
  const results = $derived(
    (found ?? library.allPeople)
      .filter((p) => !excluded.has(p.id))
      .slice(0, LIMIT),
  );

  function onInput(value: string): void {
    query = value;
    if (timer !== undefined) clearTimeout(timer);
    if (value.trim() === "") {
      found = null;
      return;
    }
    timer = setTimeout(() => void run(value), DEBOUNCE_MS);
  }

  async function run(q: string): Promise<void> {
    try {
      // `search` now returns ranked SearchHit[]; the picker shows the people.
      const hits = await api.search(q, LIMIT);
      found = hits.map((h) => h.individual);
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }
</script>

<div class="picker">
  <input
    type="search"
    value={query}
    oninput={(e) => onInput(e.currentTarget.value)}
    {placeholder}
    aria-label={placeholder}
    autocomplete="off"
  />
  <ul class="results" role="list">
    {#each results as person (person.id)}
      <li>
        <button type="button" class="result" onclick={() => onpick(person)}>
          <span class="name">{displayName(person)}</span>
          <span class="sex">{person.sex}</span>
        </button>
      </li>
    {:else}
      <li class="hint">
        {query.trim() ? `No people match “${query}”.` : "No people to pick."}
      </li>
    {/each}
  </ul>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  input {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: var(--color-paper);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .results {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 12rem;
    overflow-y: auto;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .result {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    width: 100%;
    text-align: left;
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--color-hairline);
    color: var(--color-ink);
  }

  .result:hover {
    background: var(--color-surface-2);
  }

  .sex {
    flex: none;
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .hint {
    padding: var(--space-3);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }
</style>
