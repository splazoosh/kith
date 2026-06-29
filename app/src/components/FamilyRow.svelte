<script lang="ts">
  // FamilyRow — one family in the Library list: the resolved partner label
  // ("Doe × Roe") and the union type. Child counts need a per-family load
  // (family_get), so they appear in the read-only preview, not the list row.

  import { familyLabel } from "../lib/format";
  import type { Family, Individual } from "../lib/types";

  interface Props {
    family: Family;
    peopleById: Map<number, Individual>;
    selected: boolean;
    onselect: () => void;
  }
  let { family, peopleById, selected, onselect }: Props = $props();

  const label = $derived(familyLabel(family, peopleById));
</script>

<button
  class="row"
  class:selected
  type="button"
  aria-pressed={selected}
  onclick={onselect}
>
  <span class="name">{label}</span>
  <span class="meta">{family.union_type}</span>
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
    flex: none;
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }
</style>
