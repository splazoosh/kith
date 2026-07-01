<script lang="ts">
  // EventEditor — add OR edit an event inline in the detail pane. The subject is
  // derived from context (the person/family being viewed) and is immutable. The
  // kind is a <select> of the eight known variants plus an "Other…" reveal that
  // emits `{ Other: text }`; the date rides as a RAW string through `DateInput`
  // (the command parses it); the place is a single optional text field.
  //
  // Edit mode (an `editId` is given): the existing event is loaded, its fields
  // seeded, and Save routes through `event_update`. The stored date is a
  // structured value on the wire, so it is turned back into an editable raw
  // string by the `format_date` command (the inverse of `parse_date`) — dates
  // are never formatted in the frontend. The event's original place id is
  // remembered so an unchanged place re-uses that row instead of inserting a
  // duplicate; a new place name inserts one (no dedup, as on add).

  import { onMount, untrack } from "svelte";

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import {
    eventKindLabel,
    eventKindSelect,
    KNOWN_EVENT_KINDS,
    OTHER_KIND,
    parseEventKind,
  } from "../lib/drafts";
  import { toast } from "../lib/stores/toast.svelte";
  import type { Event, EventSubject } from "../lib/types";
  import DateInput from "./DateInput.svelte";

  interface Props {
    /** The individual or family the event belongs to (immutable). */
    subject: EventSubject;
    /** When set, edit this existing event (load + seed + `event_update`)
     *  instead of adding a new one. */
    editId?: number | null;
    onsaved: (event: Event) => void;
    oncancel: () => void;
  }
  let { subject, editId = null, onsaved, oncancel }: Props = $props();

  // The event to edit is fixed for this mount; snapshot the prop (like
  // `PersonForm`) so reads stay non-reactive and never chase later prop churn.
  const editId0 = untrack(() => editId);
  const editing = editId0 !== null;

  let kindSelect = $state<string>("Birth");
  let otherText = $state("");
  let dateRaw = $state("");
  let placeName = $state("");
  let notes = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  // Edit mode loads the current values before the form is usable.
  let loading = $state(editing);
  // The event's existing place, remembered so an unchanged place re-uses the
  // same row on save (rather than the add path's "always insert a new place").
  let originalPlaceId: number | null = null;
  let originalPlaceName = "";

  // In edit mode, load the event and seed every field. The date is turned back
  // into a raw string by the core (`format_date`) — never formatted in TS.
  onMount(() => {
    if (editId0 === null) return;
    void seedFromEvent(editId0);
  });

  async function seedFromEvent(id: number): Promise<void> {
    try {
      const view = await api.eventGet(id);
      kindSelect = eventKindSelect(view.event.kind);
      otherText =
        typeof view.event.kind === "string" ? "" : view.event.kind.Other;
      dateRaw = view.event.date ? await api.formatDate(view.event.date) : "";
      placeName = view.place?.name ?? "";
      notes = view.event.notes ?? "";
      originalPlaceId = view.place?.id ?? null;
      originalPlaceName = view.place?.name ?? "";
    } catch (err) {
      toast.pushError(asCommandError(err));
      oncancel();
    } finally {
      loading = false;
    }
  }

  /** The `place_id` / `place_name` pair for the write. An unchanged place keeps
   *  its existing id (no duplicate row); a new non-empty name inserts one. */
  function resolvePlaceArgs(): {
    place_id: number | null;
    place_name: string | null;
  } {
    const trimmed = placeName.trim();
    if (trimmed === originalPlaceName.trim()) {
      // Unchanged (including "still blank") — re-use the existing place, if any.
      return { place_id: originalPlaceId, place_name: null };
    }
    if (trimmed === "") {
      // Cleared. `event_update` overlays place (it cannot unset one), so this
      // leaves the existing place; on the add path there is simply none.
      return { place_id: null, place_name: null };
    }
    return { place_id: null, place_name: trimmed };
  }

  async function submit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    busy = true;
    try {
      const kind = parseEventKind(kindSelect, otherText);
      // Empty ⇒ undated (null). A present-but-blank string is a validation
      // error in the core, so never send "".
      const date = dateRaw.trim() === "" ? null : dateRaw;
      const notesArg = notes.trim() === "" ? null : notes;
      let saved: Event;
      if (editId0 !== null) {
        saved = await api.eventUpdate({
          id: editId0,
          kind,
          date,
          ...resolvePlaceArgs(),
          notes: notesArg,
        });
      } else {
        saved = await api.eventAdd({
          subject,
          kind,
          date,
          place_id: null,
          place_name: placeName.trim() === "" ? null : placeName.trim(),
          notes: notesArg,
        });
      }
      onsaved(saved);
    } catch (err) {
      // Validation (e.g. a forced bad date) teaches inline; anything else toasts.
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

<form class="event-editor" onsubmit={submit}>
  {#if loading}
    <p class="hint">Loading…</p>
  {:else}
  <div class="field">
    <label for="event-kind">Kind</label>
    <select id="event-kind" bind:value={kindSelect}>
      {#each KNOWN_EVENT_KINDS as k (k)}
        <option value={k}>{eventKindLabel(k)}</option>
      {/each}
      <option value={OTHER_KIND}>Other…</option>
    </select>
  </div>

  {#if kindSelect === OTHER_KIND}
    <div class="field">
      <label for="event-other">Other kind</label>
      <input
        id="event-other"
        type="text"
        bind:value={otherText}
        placeholder="e.g. christening"
      />
    </div>
  {/if}

  <DateInput bind:value={dateRaw} label="Date" id="event-date" />

  <div class="field">
    <label for="event-place">Place</label>
    <input
      id="event-place"
      type="text"
      bind:value={placeName}
      placeholder="e.g. Bergen"
    />
  </div>

  <div class="field">
    <label for="event-notes">Notes</label>
    <textarea id="event-notes" rows="2" bind:value={notes}></textarea>
  </div>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <div class="actions">
    <button type="button" onclick={oncancel}>Cancel</button>
    <button type="submit" class="primary" disabled={busy}>
      {editing ? "Save changes" : "Add event"}
    </button>
  </div>
  {/if}
</form>

<style>
  .event-editor {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    background: var(--color-surface-2);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  label {
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .error {
    margin: 0;
    color: var(--color-danger);
    font-size: var(--text-sm);
  }

  .hint {
    margin: 0;
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
