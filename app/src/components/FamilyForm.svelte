<script lang="ts">
  // FamilyForm — create or edit one family. CREATE bulk-fills a
  // `NewFamily` from up to two `PersonPicker` slots; EDIT is INCREMENTAL,
  // mapping each action onto the server-side slot/append rules the client
  // never re-derives: `family_add_partner` fills the first empty slot (a third
  // is rejected as `validation`, surfaced inline), `family_add_child` appends,
  // `family_remove_child` removes, and clearing a partner / changing union/notes
  // goes through `family_update`. Edit reads the resolved members from the
  // `FamilyView` and reports each write through `onchanged` so the detail pane
  // reloads; create reports through `onsaved`.

  import { untrack } from "svelte";

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { displayName } from "../lib/format";
  import { toast } from "../lib/stores/toast.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type {
    ChildRelation,
    Family,
    FamilyView,
    Individual,
    NewFamily,
    UnionType,
  } from "../lib/types";
  import PersonPicker from "./PersonPicker.svelte";

  interface Props {
    /** Present ⇒ edit; absent ⇒ create. */
    family?: Family;
    /** The resolved members, for the edit view (partners + children). */
    view?: FamilyView;
    onsaved: (saved: Family) => void;
    oncancel: () => void;
    /** Called after an incremental edit write so the detail pane reloads. */
    onchanged: () => void;
  }
  let { family, view, onsaved, oncancel, onchanged }: Props = $props();

  const UNIONS: readonly UnionType[] = ["Marriage", "Partnership", "Unknown"];
  const RELATIONS: readonly ChildRelation[] = ["Birth", "Adopted", "Step", "Foster"];
  // Seed the editable core fields ONCE; incremental edits go straight to
  // the commands, so prop churn from a reselect must not reset these.
  const seed = untrack(() => family);
  const isEdit = seed !== undefined;
  let unionType = $state<UnionType>(seed?.union_type ?? "Unknown");
  let notes = $state(seed?.notes ?? "");
  let error = $state<string | null>(null);
  let busy = $state(false);

  // — create-only local picks —
  let partner1 = $state<Individual | null>(null);
  let partner2 = $state<Individual | null>(null);

  // — edit-only "add child" sub-state —
  let addingChild = $state(false);
  let childRelation = $state<ChildRelation>("Birth");

  function blankToNull(s: string): string | null {
    const t = s.trim();
    return t === "" ? null : t;
  }

  // A guarded write wrapper for the incremental edit actions.
  async function run(action: () => Promise<unknown>): Promise<void> {
    error = null;
    busy = true;
    try {
      await action();
      onchanged();
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

  // — create —
  async function create(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    busy = true;
    try {
      const draft: NewFamily = {
        partner1: partner1?.id ?? null,
        partner2: partner2?.id ?? null,
        union_type: unionType,
        notes: blankToNull(notes),
      };
      const saved = await api.familyCreate(draft);
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

  // — edit: details / partners / children —
  function saveDetails(): void {
    if (!family) return;
    const record: Family = { ...family, union_type: unionType, notes: blankToNull(notes) };
    void run(() => api.familyUpdate(record));
  }

  function clearPartner(slot: 1 | 2): void {
    if (!family) return;
    const record: Family =
      slot === 1 ? { ...family, partner1: null } : { ...family, partner2: null };
    void run(() => api.familyUpdate(record));
  }

  function addPartner(person: Individual): void {
    if (!family) return;
    void run(() => api.familyAddPartner(family.id, person.id));
  }

  function addChild(person: Individual): void {
    if (!family) return;
    const relation = childRelation;
    addingChild = false;
    void run(() => api.familyAddChild(family.id, person.id, relation));
  }

  function removeChild(childId: number, label: string): void {
    if (!family) return;
    void run(async () => {
      await api.familyRemoveChild(family.id, childId);
      undo.recordDelete(`${label} from the family`);
    });
  }

  // Ids already linked (partners + children) — excluded from the pickers.
  const memberIds = $derived(
    view
      ? [
          ...(view.partner1 ? [view.partner1.id] : []),
          ...(view.partner2 ? [view.partner2.id] : []),
          ...view.children.map((c) => c.child_id),
        ]
      : [],
  );
</script>

{#if !isEdit}
  <!-- CREATE -->
  <form class="family-form" onsubmit={create}>
    <h2 class="title">New family</h2>

    <label class="field">
      <span>Union type</span>
      <select bind:value={unionType}>
        {#each UNIONS as u (u)}
          <option value={u}>{u}</option>
        {/each}
      </select>
    </label>

    <div class="slots">
      <div class="slot">
        <span class="slot-label">Partner 1</span>
        {#if partner1}
          <div class="chosen">
            <span class="name">{displayName(partner1)}</span>
            <button type="button" class="link" onclick={() => (partner1 = null)}>
              Clear
            </button>
          </div>
        {:else}
          <PersonPicker
            onpick={(p) => (partner1 = p)}
            exclude={partner2 ? [partner2.id] : []}
            placeholder="Pick partner 1…"
          />
        {/if}
      </div>

      <div class="slot">
        <span class="slot-label">Partner 2</span>
        {#if partner2}
          <div class="chosen">
            <span class="name">{displayName(partner2)}</span>
            <button type="button" class="link" onclick={() => (partner2 = null)}>
              Clear
            </button>
          </div>
        {:else}
          <PersonPicker
            onpick={(p) => (partner2 = p)}
            exclude={partner1 ? [partner1.id] : []}
            placeholder="Pick partner 2…"
          />
        {/if}
      </div>
    </div>

    <label class="field">
      <span>Notes</span>
      <textarea rows="2" bind:value={notes}></textarea>
    </label>

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="actions">
      <button type="button" onclick={oncancel}>Cancel</button>
      <button type="submit" class="primary" disabled={busy}>Create family</button>
    </div>
  </form>
{:else if family && view}
  <!-- EDIT -->
  <div class="family-form">
    <h2 class="title">Edit family</h2>

    <div class="details">
      <label class="field">
        <span>Union type</span>
        <select bind:value={unionType}>
          {#each UNIONS as u (u)}
            <option value={u}>{u}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span>Notes</span>
        <textarea rows="2" bind:value={notes}></textarea>
      </label>
      <button type="button" class="primary" disabled={busy} onclick={saveDetails}>
        Save details
      </button>
    </div>

    <h3>Partners</h3>
    <div class="slots">
      <div class="slot">
        <span class="slot-label">Partner 1</span>
        {#if view.partner1}
          <div class="chosen">
            <span class="name">{displayName(view.partner1)}</span>
            <button type="button" class="link" onclick={() => clearPartner(1)}>
              Clear
            </button>
          </div>
        {:else}
          <!-- add fills the first empty slot server-side -->
          <PersonPicker onpick={addPartner} exclude={memberIds} placeholder="Add a partner…" />
        {/if}
      </div>

      <div class="slot">
        <span class="slot-label">Partner 2</span>
        {#if view.partner2}
          <div class="chosen">
            <span class="name">{displayName(view.partner2)}</span>
            <button type="button" class="link" onclick={() => clearPartner(2)}>
              Clear
            </button>
          </div>
        {:else if !view.partner1}
          <span class="hint">Add partner 1 first.</span>
        {:else}
          <PersonPicker onpick={addPartner} exclude={memberIds} placeholder="Add a partner…" />
        {/if}
      </div>
    </div>

    <h3>Children</h3>
    {#if view.children.length > 0}
      <ul class="children" role="list">
        {#each view.children as child (child.child_id)}
          <li>
            <span class="name">{displayName(child.individual)}</span>
            <span class="rel">{child.relation}</span>
            <button
              type="button"
              class="link danger"
              onclick={() => removeChild(child.child_id, displayName(child.individual))}
            >
              Remove
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="hint">No children.</p>
    {/if}

    {#if addingChild}
      <div class="add-child">
        <label class="field">
          <span>Relation</span>
          <select bind:value={childRelation}>
            {#each RELATIONS as r (r)}
              <option value={r}>{r}</option>
            {/each}
          </select>
        </label>
        <PersonPicker onpick={addChild} exclude={memberIds} placeholder="Pick a child…" />
        <div class="actions">
          <button type="button" onclick={() => (addingChild = false)}>Cancel</button>
        </div>
      </div>
    {:else}
      <button type="button" class="link" onclick={() => (addingChild = true)}>
        + Add child
      </button>
    {/if}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="actions done">
      <button type="button" class="primary" onclick={oncancel}>Done</button>
    </div>
  </div>
{/if}

<style>
  .family-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .title {
    font-size: var(--text-xl);
  }

  h3 {
    font-size: var(--text-md);
    color: var(--color-ink-soft);
    margin-top: var(--space-2);
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: flex-start;
  }

  .details .primary {
    align-self: flex-end;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    width: 100%;
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .slots {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-4);
  }

  .slot {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .slot-label {
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .chosen {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--color-surface-2);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .children {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .children li {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--color-hairline);
  }

  .children .name {
    flex: 1;
  }

  .rel {
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .add-child {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    background: var(--color-surface-2);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
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

  .actions.done {
    border-top: 1px solid var(--color-hairline);
    padding-top: var(--space-4);
  }

  .link {
    background: transparent;
    border: none;
    color: var(--color-accent-text);
    font-size: var(--text-sm);
    padding: 0;
    align-self: flex-start;
  }

  .link.danger {
    color: var(--color-danger);
  }
</style>
