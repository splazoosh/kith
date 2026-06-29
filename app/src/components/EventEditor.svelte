<script lang="ts">
  // EventEditor — add an event inline in the detail pane. The subject is
  // derived from context (the person/family being viewed) and is immutable. The
  // kind is a <select> of the eight known variants plus an "Other…" reveal that
  // emits `{ Other: text }`; the date rides as a RAW string through `DateInput`
  // (the command parses it); the place is a single optional text field →
  // `place_name` (no place_id/dedup).
  //
  // Add + Delete only: an existing event's date is stored canonically and is not
  // exposed as a raw string on the wire, and `event_update` REPLACES the date —
  // so a lossless in-place edit is impossible without reformatting dates in TS
  // (forbidden) or a backend change (out of scope). Editing an event is
  // therefore remove-and-re-add.

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import {
    eventKindLabel,
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
    onsaved: (event: Event) => void;
    oncancel: () => void;
  }
  let { subject, onsaved, oncancel }: Props = $props();

  let kindSelect = $state<string>("Birth");
  let otherText = $state("");
  let dateRaw = $state("");
  let placeName = $state("");
  let notes = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    busy = true;
    try {
      const saved = await api.eventAdd({
        subject,
        kind: parseEventKind(kindSelect, otherText),
        // Empty ⇒ undated (null). A present-but-blank string is a validation
        // error in the core, so never send "".
        date: dateRaw.trim() === "" ? null : dateRaw,
        place_id: null,
        place_name: placeName.trim() === "" ? null : placeName.trim(),
        notes: notes.trim() === "" ? null : notes,
      });
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
    <button type="submit" class="primary" disabled={busy}>Add event</button>
  </div>
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

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }
</style>
