<script lang="ts">
  // Library — the browse surface: People / Families tabs, a search box, and the
  // sorted list. People search is server-side+debounced; families filter
  // client-side (both driven through the shared `library.query`). Selecting
  // a row loads its read-only view into the selection store.

  import { library } from "../lib/stores/library.svelte";
  import { selection } from "../lib/stores/selection.svelte";
  import EmptyState from "./EmptyState.svelte";
  import FamilyRow from "./FamilyRow.svelte";
  import PersonRow from "./PersonRow.svelte";

  type Tab = "people" | "families";
  let tab = $state<Tab>("people");

  // The stable, name-ordered people list is computed once-per-change in the store
  // (`library.sortedPeople`), not re-sorted here on every render.

  const isEmpty = $derived(
    library.allPeople.length === 0 && library.allFamilies.length === 0,
  );

  function onSearch(value: string): void {
    library.setQuery(value);
  }
</script>

{#if isEmpty}
  <EmptyState mode="no-people" />
{:else}
  <section class="library" aria-label="Library">
    <div class="tabs" role="tablist" aria-label="Record kind">
      <button
        role="tab"
        type="button"
        aria-selected={tab === "people"}
        class:active={tab === "people"}
        onclick={() => (tab = "people")}
      >
        People <span class="count">{library.allPeople.length}</span>
      </button>
      <button
        role="tab"
        type="button"
        aria-selected={tab === "families"}
        class:active={tab === "families"}
        onclick={() => (tab = "families")}
      >
        Families <span class="count">{library.allFamilies.length}</span>
      </button>
    </div>

    <div class="search">
      <input
        type="search"
        value={library.query}
        oninput={(e) => onSearch(e.currentTarget.value)}
        placeholder={tab === "people" ? "Search people…" : "Filter families…"}
        aria-label={tab === "people" ? "Search people" : "Filter families"}
      />
      <button
        type="button"
        class="new"
        onclick={() => selection.startCreate(tab === "people" ? "person" : "family")}
      >
        {tab === "people" ? "+ New person" : "+ New family"}
      </button>
    </div>

    <div class="list" role="list">
      {#if tab === "people"}
        {#each library.sortedPeople as person (person.id)}
          <PersonRow
            {person}
            selected={selection.current?.kind === "person" &&
              selection.current.id === person.id}
            onselect={() => selection.selectPerson(person.id)}
          />
        {:else}
          <p class="hint">
            {#if library.query.trim()}
              No people match “{library.query}”.
            {:else}
              No people.
            {/if}
          </p>
        {/each}
      {:else}
        {#each library.families as family (family.id)}
          <FamilyRow
            {family}
            peopleById={library.peopleById}
            selected={selection.current?.kind === "family" &&
              selection.current.id === family.id}
            onselect={() => selection.selectFamily(family.id)}
          />
        {:else}
          <p class="hint">
            {#if library.query.trim()}
              No families match “{library.query}”.
            {:else}
              No families.
            {/if}
          </p>
        {/each}
      {/if}
    </div>
  </section>
{/if}

<style>
  .library {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    border-right: 1px solid var(--color-hairline);
    background: var(--color-surface);
  }

  .tabs {
    display: flex;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3) 0;
    border-bottom: 1px solid var(--color-hairline);
  }

  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .tabs button.active {
    color: var(--color-ink);
    border-bottom-color: var(--color-accent);
  }

  .count {
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
    background: var(--color-surface-2);
    border-radius: 999px;
    padding: 0 var(--space-2);
  }

  .search {
    display: flex;
    gap: var(--space-2);
    padding: var(--space-3);
  }

  .search input {
    flex: 1;
    min-width: 0;
    padding: var(--space-2) var(--space-3);
    background: var(--color-paper);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .search .new {
    flex: none;
    padding: var(--space-2) var(--space-3);
    background: var(--color-accent);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    color: var(--color-surface);
    font-size: var(--text-sm);
    white-space: nowrap;
  }

  .search .new:hover {
    background: var(--color-accent-text);
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .hint {
    padding: var(--space-4);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }
</style>
