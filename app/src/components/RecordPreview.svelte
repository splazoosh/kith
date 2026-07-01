<script lang="ts">
  // RecordPreview — the detail pane. Around the read-only summary it
  // now hosts the write surface: Edit / Delete actions, the inline EventEditor
  // and NamesEditor in view mode, and the PersonForm / FamilyForm in edit or
  // create mode. The forms are self-contained and report back through
  // callbacks; this component owns the store wiring (reload, (re)select, mode).
  // Cascading deletes go through the in-app ConfirmDialog — no native dialog, no
  // ACL change. Errors from the forms surface inline; store writes
  // here toast on failure (never a white-screen).

  import * as api from "../lib/api";
  import { eventKindLabel } from "../lib/drafts";
  import { asCommandError } from "../lib/errors";
  import { dateYear, displayName, lifespanYears } from "../lib/format";
  import { chart } from "../lib/stores/chart.svelte";
  import { library } from "../lib/stores/library.svelte";
  import { selection } from "../lib/stores/selection.svelte";
  import { toast } from "../lib/stores/toast.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type { Family, FamilyView, Individual } from "../lib/types";
  import CitationsEditor from "./CitationsEditor.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import EventEditor from "./EventEditor.svelte";
  import FamilyForm from "./FamilyForm.svelte";
  import MediaGallery from "./MediaGallery.svelte";
  import NamesEditor from "./NamesEditor.svelte";
  import PersonForm from "./PersonForm.svelte";

  let addingEvent = $state(false);
  let confirming = $state<"person" | "family" | null>(null);
  // The event whose citations editor is open (one at a time); events-only.
  let citingEvent = $state<number | null>(null);
  // The event being edited in place (one at a time), mutually exclusive with the
  // citations editor and the add-event form.
  let editingEvent = $state<number | null>(null);

  // Reset the transient detail UI whenever the selected record changes (which
  // includes the in-place reselect after a sub-editor write — so the add/edit
  // event form closes once its event lands).
  $effect(() => {
    selection.current?.kind;
    selection.current?.id;
    addingEvent = false;
    confirming = null;
    citingEvent = null;
    editingEvent = null;
  });

  function toggleCiting(id: number): void {
    editingEvent = null;
    citingEvent = citingEvent === id ? null : id;
  }

  function startEditEvent(id: number): void {
    addingEvent = false;
    citingEvent = null;
    editingEvent = id;
  }

  function familyTitle(v: FamilyView): string {
    const names = [v.partner1, v.partner2]
      .filter((p): p is Individual => p !== null)
      .map((p) => displayName(p));
    return names.length > 0 ? names.join(" × ") : "(unlinked family)";
  }

  // — form callbacks —
  async function onPersonSaved(saved: Individual): Promise<void> {
    await library.reload();
    await selection.selectPerson(saved.id); // resets to view mode
  }

  async function onFamilySaved(saved: Family): Promise<void> {
    await library.reload();
    await selection.selectFamily(saved.id);
  }

  // An incremental family edit (partner/child/details) — refresh in place,
  // staying in edit mode; partner changes can move family labels, so reload.
  async function onFamilyChanged(): Promise<void> {
    await selection.reselect();
    await library.reload();
  }

  function cancelForm(): void {
    selection.endEdit();
  }

  // — events (add / edit in place / remove) —
  async function onEventSaved(): Promise<void> {
    addingEvent = false;
    editingEvent = null;
    await selection.reselect();
  }

  async function removeEvent(id: number, label: string): Promise<void> {
    try {
      await api.eventDelete(id);
      undo.recordDelete(`the ${label} event`);
      await selection.reselect();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  // — cascading delete (gated by ConfirmDialog) —
  async function confirmDelete(): Promise<void> {
    const c = selection.current;
    if (c === null) return;
    confirming = null;
    const label =
      c.kind === "person" ? displayName(c.view.individual) : familyTitle(c.view);
    try {
      if (c.kind === "person") await api.personDelete(c.id);
      else await api.familyDelete(c.id);
      undo.recordDelete(label);
      selection.clear();
      await library.reload();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  const deleteBody = $derived(
    selection.current?.kind === "family"
      ? "This permanently removes the family and its events, and unlinks its partners and children. The people themselves are kept."
      : "This permanently removes the person, their alternate names and events, and removes them from any family.",
  );
</script>

<aside class="preview" aria-label="Record details">
  {#if selection.creating === "person"}
    <PersonForm onsaved={onPersonSaved} oncancel={cancelForm} />
  {:else if selection.creating === "family"}
    <FamilyForm onsaved={onFamilySaved} oncancel={cancelForm} onchanged={onFamilyChanged} />
  {:else if selection.loading}
    <p class="hint">Loading…</p>
  {:else if selection.current === null}
    <p class="hint">Select a person or family to see its details.</p>
  {:else if selection.mode === "edit" && selection.current.kind === "person"}
    <PersonForm
      person={selection.current.view.individual}
      onsaved={onPersonSaved}
      oncancel={cancelForm}
    />
  {:else if selection.mode === "edit" && selection.current.kind === "family"}
    {@const v = selection.current.view}
    <FamilyForm
      family={v.family}
      view={v}
      onsaved={onFamilySaved}
      oncancel={cancelForm}
      onchanged={onFamilyChanged}
    />
  {:else if selection.current.kind === "person"}
    {@const v = selection.current.view}
    {@const span = lifespanYears(v.events)}
    <div class="head">
      <h2 class="name title">{displayName(v.individual)}</h2>
      <div class="record-actions">
        <!-- Seed the chart root on this person, then switch to the tree view
             — the chart loads while the view flips in. -->
        <button
          type="button"
          onclick={() => {
            chart.view(v.individual.id);
            ui.showTree();
          }}
        >
          View in tree
        </button>
        <button type="button" onclick={() => selection.startEdit()}>Edit</button>
        <button type="button" class="danger" onclick={() => (confirming = "person")}>
          Delete
        </button>
      </div>
    </div>
    <dl>
      <div><dt>Sex</dt><dd>{v.individual.sex}</dd></div>
      <div>
        <dt>Status</dt>
        <dd>{v.individual.living ? "Living" : "Deceased"}</dd>
      </div>
      {#if span}
        <div><dt>Years</dt><dd>{span}</dd></div>
      {/if}
      <div><dt>Families (as partner)</dt><dd>{v.partner_in.length}</dd></div>
      <div><dt>Families (as child)</dt><dd>{v.child_in.length}</dd></div>
    </dl>
    {#if v.individual.notes}
      <p class="notes">{v.individual.notes}</p>
    {/if}

    <section class="events">
      <div class="section-head">
        <h3>Events</h3>
        {#if !addingEvent}
          <button type="button" class="link" onclick={() => (addingEvent = true)}>
            + Add event
          </button>
        {/if}
      </div>
      {#if v.events.length > 0}
        <ul role="list">
          {#each v.events as ev (ev.id)}
            <li>
              <div class="ev-row">
                <span class="ev-kind">{eventKindLabel(ev.kind)}</span>
                {#if dateYear(ev.date)}<span class="ev-year">{dateYear(ev.date)}</span>{/if}
                <button type="button" class="link" onclick={() => startEditEvent(ev.id)}>
                  Edit
                </button>
                <button
                  type="button"
                  class="link"
                  aria-expanded={citingEvent === ev.id}
                  onclick={() => toggleCiting(ev.id)}
                >
                  Sources
                </button>
                <button
                  type="button"
                  class="link danger"
                  onclick={() => removeEvent(ev.id, eventKindLabel(ev.kind))}
                >
                  Remove
                </button>
              </div>
              {#if editingEvent === ev.id}
                <EventEditor
                  subject={ev.subject}
                  editId={ev.id}
                  onsaved={onEventSaved}
                  oncancel={() => (editingEvent = null)}
                />
              {:else if citingEvent === ev.id}
                <CitationsEditor subject={{ Event: ev.id }} />
              {/if}
            </li>
          {/each}
        </ul>
      {:else if !addingEvent}
        <p class="hint">No events.</p>
      {/if}
      {#if addingEvent}
        <EventEditor
          subject={{ Individual: v.individual.id }}
          onsaved={onEventSaved}
          oncancel={() => (addingEvent = false)}
        />
      {/if}
    </section>

    <NamesEditor individualId={v.individual.id} />

    <MediaGallery subject={{ Individual: v.individual.id }} />
  {:else}
    {@const v = selection.current.view}
    <div class="head">
      <h2 class="name title">{familyTitle(v)}</h2>
      <div class="record-actions">
        <button type="button" onclick={() => selection.startEdit()}>Edit</button>
        <button type="button" class="danger" onclick={() => (confirming = "family")}>
          Delete
        </button>
      </div>
    </div>
    <dl>
      <div><dt>Union</dt><dd>{v.family.union_type}</dd></div>
      <div>
        <dt>Partner 1</dt>
        <dd>{v.partner1 ? displayName(v.partner1) : "—"}</dd>
      </div>
      <div>
        <dt>Partner 2</dt>
        <dd>{v.partner2 ? displayName(v.partner2) : "—"}</dd>
      </div>
    </dl>
    {#if v.children.length > 0}
      <h3>Children</h3>
      <ul class="children" role="list">
        {#each v.children as child (child.child_id)}
          <li>
            <span class="name">{displayName(child.individual)}</span>
            <span class="rel">{child.relation}</span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if v.family.notes}
      <p class="notes">{v.family.notes}</p>
    {/if}

    <section class="events">
      <div class="section-head">
        <h3>Events</h3>
        {#if !addingEvent}
          <button type="button" class="link" onclick={() => (addingEvent = true)}>
            + Add event
          </button>
        {/if}
      </div>
      {#if v.events.length > 0}
        <ul role="list">
          {#each v.events as ev (ev.id)}
            <li>
              <div class="ev-row">
                <span class="ev-kind">{eventKindLabel(ev.kind)}</span>
                {#if dateYear(ev.date)}<span class="ev-year">{dateYear(ev.date)}</span>{/if}
                <button type="button" class="link" onclick={() => startEditEvent(ev.id)}>
                  Edit
                </button>
                <button
                  type="button"
                  class="link"
                  aria-expanded={citingEvent === ev.id}
                  onclick={() => toggleCiting(ev.id)}
                >
                  Sources
                </button>
                <button
                  type="button"
                  class="link danger"
                  onclick={() => removeEvent(ev.id, eventKindLabel(ev.kind))}
                >
                  Remove
                </button>
              </div>
              {#if editingEvent === ev.id}
                <EventEditor
                  subject={ev.subject}
                  editId={ev.id}
                  onsaved={onEventSaved}
                  oncancel={() => (editingEvent = null)}
                />
              {:else if citingEvent === ev.id}
                <CitationsEditor subject={{ Event: ev.id }} />
              {/if}
            </li>
          {/each}
        </ul>
      {:else if !addingEvent}
        <p class="hint">No events.</p>
      {/if}
      {#if addingEvent}
        <EventEditor
          subject={{ Family: v.family.id }}
          onsaved={onEventSaved}
          oncancel={() => (addingEvent = false)}
        />
      {/if}
    </section>
  {/if}

  {#if confirming}
    <ConfirmDialog
      title={confirming === "family" ? "Delete this family?" : "Delete this person?"}
      body={deleteBody}
      confirmLabel="Delete"
      danger
      onconfirm={confirmDelete}
      oncancel={() => (confirming = null)}
    />
  {/if}
</aside>

<style>
  .preview {
    height: 100%;
    overflow-y: auto;
    padding: var(--space-6);
    background: var(--color-paper);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-4);
  }

  .title {
    font-size: var(--text-xl);
  }

  .record-actions {
    display: flex;
    gap: var(--space-2);
    flex: none;
  }

  .record-actions button {
    padding: var(--space-1) var(--space-3);
    background: transparent;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
    font-size: var(--text-sm);
  }

  .record-actions button:hover {
    border-color: var(--color-accent);
  }

  .record-actions button.danger:hover {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0;
  }

  dl > div {
    display: contents;
  }

  dt {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  dd {
    margin: 0;
    color: var(--color-ink);
  }

  h3 {
    margin-top: var(--space-6);
    margin-bottom: var(--space-2);
    font-size: var(--text-md);
    color: var(--color-ink-soft);
  }

  .section-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-top: var(--space-6);
  }

  .section-head h3 {
    margin: 0;
  }

  .events ul,
  .children {
    list-style: none;
    margin: var(--space-2) 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .events li,
  .children li {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: var(--space-1) 0;
    border-bottom: 1px solid var(--color-hairline);
  }

  /* An event row stacks its (horizontal) summary row above an inline citations
     editor when expanded. */
  .events li {
    flex-direction: column;
    align-items: stretch;
  }

  .ev-row {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  .ev-kind,
  .children .name {
    flex: 1;
  }

  .ev-year,
  .rel {
    flex: none;
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  .notes {
    margin-top: var(--space-6);
    padding-top: var(--space-4);
    border-top: 1px solid var(--color-hairline);
    color: var(--color-ink-soft);
    white-space: pre-wrap;
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
