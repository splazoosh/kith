<script lang="ts">
  // JumpToPerson — the global jump-to-person command palette. A
  // centered modal: a search field over the WHOLE tree + a ranked, keyboard-
  // navigable result list (↑/↓ to move, Enter to jump, Esc to close). All
  // ranking lives server-side — this renders the SearchHit[] the palette store
  // holds and adds interaction only. Renders nothing when closed. The focus +
  // backdrop wiring mirrors ExportDialog.

  import { tick } from "svelte";

  import { displayName } from "../lib/format";
  import { modal } from "../lib/stores/modal.svelte";
  import { searchPalette } from "../lib/stores/search.svelte";

  let input = $state<HTMLInputElement | null>(null);

  // Autofocus the input each time the palette opens (the modal owns focus).
  $effect(() => {
    if (searchPalette.isOpen) void tick().then(() => input?.focus());
  });

  // Register as a modal while open, so the shortcut registry stands down (the
  // palette keeps its own ↑/↓/Enter/Esc handling).
  $effect(() => {
    if (searchPalette.isOpen) return modal.open();
  });

  function onKeydown(e: KeyboardEvent): void {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        searchPalette.close();
        break;
      case "ArrowDown":
        e.preventDefault();
        searchPalette.move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        searchPalette.move(-1);
        break;
      case "Enter":
        e.preventDefault();
        searchPalette.chooseSelected();
        break;
    }
  }
</script>

{#if searchPalette.isOpen}
  <!-- The backdrop: a click outside the palette closes it. -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) searchPalette.close();
    }}
  >
    <div class="palette" role="dialog" aria-modal="true" aria-label="Jump to person">
      <input
        bind:this={input}
        type="search"
        value={searchPalette.query}
        oninput={(e) => searchPalette.setQuery(e.currentTarget.value)}
        onkeydown={onKeydown}
        placeholder="Jump to a person…"
        aria-label="Search people"
      />
      <ul class="results" role="listbox" aria-label="Search results">
        {#each searchPalette.hits as hit, i (hit.individual.id)}
          <li role="option" aria-selected={i === searchPalette.selectedIndex}>
            <button
              type="button"
              class:active={i === searchPalette.selectedIndex}
              onclick={() => searchPalette.choose(hit)}
              onmousemove={() => (searchPalette.selectedIndex = i)}
            >
              <span class="name">{displayName(hit.individual)}</span>
              {#if hit.context}
                <span class="context">{hit.context}</span>
              {/if}
            </button>
          </li>
        {:else}
          <li class="hint">
            {#if searchPalette.query.trim()}
              No people match “{searchPalette.query}”.
            {:else}
              Type to search names, places, and notes…
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 12vh var(--space-4) var(--space-4);
    background: rgba(0, 0, 0, 0.45);
  }

  .palette {
    width: min(34rem, 100%);
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-2);
    overflow: hidden;
  }

  input {
    padding: var(--space-3) var(--space-4);
    background: var(--color-paper);
    border: none;
    border-bottom: 1px solid var(--color-hairline);
    color: var(--color-ink);
    font-size: var(--text-md);
  }

  .results {
    list-style: none;
    margin: 0;
    padding: var(--space-1);
    overflow-y: auto;
  }

  .results button {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-radius: var(--radius-md);
    color: var(--color-ink);
    text-align: left;
  }

  .results button.active {
    background: var(--color-accent-weak);
  }

  .name {
    font-weight: 500;
  }

  .context {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    padding: var(--space-4);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }
</style>
