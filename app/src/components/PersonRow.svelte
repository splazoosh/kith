<script lang="ts">
  // PersonRow — one individual in the Library list: serif display name, sex, a
  // "living" badge. A focusable button; selection is reflected via aria-pressed.

  import { displayName } from "../lib/format";
  import type { Individual } from "../lib/types";

  interface Props {
    person: Individual;
    selected: boolean;
    onselect: () => void;
  }
  let { person, selected, onselect }: Props = $props();
</script>

<button
  class="row"
  class:selected
  type="button"
  aria-pressed={selected}
  onclick={onselect}
>
  <span class="name">{displayName(person)}</span>
  <span class="meta">
    <span class="sex">{person.sex}</span>
    {#if person.living}
      <span class="badge">living</span>
    {/if}
  </span>
</button>

<style>
  .row {
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
    transition: background var(--motion-fast);
  }

  .row:hover {
    background: var(--color-surface-2);
  }

  .row.selected {
    background: var(--color-accent-weak);
  }

  .name {
    font-size: var(--text-md);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    flex: none;
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .badge {
    padding: 0 var(--space-2);
    border: 1px solid var(--color-hairline);
    border-radius: 999px;
    color: var(--color-accent-text);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
</style>
