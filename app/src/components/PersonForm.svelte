<script lang="ts">
  // PersonForm — create or edit one individual. One component for both:
  // an absent `person` prop means create (seeded from the core defaults), a
  // present one means edit (seeded from the record). The in-progress draft is
  // LOCAL state, not a store — the form is self-contained and reports
  // success through `onsaved`, so the detail pane owns the reload/select.
  //
  // Birth/death dates appear on CREATE only (the command makes the events from
  // raw strings); on EDIT this form writes the record fields only —
  // events and alternate names are managed by their own inline editors.
  // Minimal client-side blocking: an unnamed person is valid, so the only
  // failures surfaced are the command's own, narrowed by `kind`.

  import { untrack } from "svelte";

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { toast } from "../lib/stores/toast.svelte";
  import type { Individual, NewIndividual, Sex } from "../lib/types";
  import DateInput from "./DateInput.svelte";

  interface Props {
    person?: Individual;
    onsaved: (saved: Individual) => void;
    oncancel: () => void;
  }
  let { person, onsaved, oncancel }: Props = $props();

  const SEXES: readonly Sex[] = ["Male", "Female", "Other", "Unknown"];

  // Local draft, seeded ONCE from the record (edit) or the core defaults
  // (create). `untrack` snapshots the prop on init — the form owns its draft
  // from here, so later prop churn must not clobber the user's edits.
  const seed = untrack(() => person);
  const isEdit = seed !== undefined;
  let given = $state(seed?.given_name ?? "");
  let surname = $state(seed?.surname ?? "");
  let prefix = $state(seed?.name_prefix ?? "");
  let suffix = $state(seed?.name_suffix ?? "");
  let nickname = $state(seed?.nickname ?? "");
  let sex = $state<Sex>(seed?.sex ?? "Unknown");
  let living = $state(seed?.living ?? true);
  let notes = $state(seed?.notes ?? "");
  let birthRaw = $state("");
  let deathRaw = $state("");

  let error = $state<string | null>(null);
  let busy = $state(false);

  function blankToNull(s: string): string | null {
    const t = s.trim();
    return t === "" ? null : t;
  }

  function draft(): NewIndividual {
    return {
      given_name: blankToNull(given),
      surname: blankToNull(surname),
      name_prefix: blankToNull(prefix),
      name_suffix: blankToNull(suffix),
      nickname: blankToNull(nickname),
      sex,
      living,
      notes: blankToNull(notes),
    };
  }

  async function submit(e: SubmitEvent): Promise<void> {
    e.preventDefault();
    error = null;
    busy = true;
    try {
      let saved: Individual;
      if (person) {
        // Full record, id preserved; draft overrides the editable fields.
        saved = await api.personUpdate({ ...person, ...draft() });
      } else {
        saved = await api.personCreate(
          draft(),
          birthRaw.trim() === "" ? undefined : birthRaw,
          deathRaw.trim() === "" ? undefined : deathRaw,
        );
      }
      onsaved(saved);
    } catch (err) {
      // validation → teach inline; everything else → toast.
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

<form class="person-form" onsubmit={submit}>
  <h2 class="title">{isEdit ? "Edit person" : "New person"}</h2>

  <div class="grid">
    <label class="field">
      <span>Given name</span>
      <input id="person-given" type="text" bind:value={given} />
    </label>
    <label class="field">
      <span>Surname</span>
      <input id="person-surname" type="text" bind:value={surname} />
    </label>
    <label class="field">
      <span>Prefix</span>
      <input id="person-prefix" type="text" bind:value={prefix} />
    </label>
    <label class="field">
      <span>Suffix</span>
      <input id="person-suffix" type="text" bind:value={suffix} />
    </label>
    <label class="field">
      <span>Nickname</span>
      <input id="person-nickname" type="text" bind:value={nickname} />
    </label>
    <label class="field">
      <span>Sex</span>
      <select id="person-sex" bind:value={sex}>
        {#each SEXES as s (s)}
          <option value={s}>{s}</option>
        {/each}
      </select>
    </label>
  </div>

  <label class="check">
    <input type="checkbox" bind:checked={living} />
    <span>Living</span>
  </label>

  {#if !isEdit}
    <div class="dates">
      <DateInput bind:value={birthRaw} label="Birth" id="person-birth" />
      <DateInput bind:value={deathRaw} label="Death" id="person-death" />
    </div>
  {/if}

  <label class="field">
    <span>Notes</span>
    <textarea id="person-notes" rows="3" bind:value={notes}></textarea>
  </label>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <div class="actions">
    <button type="button" onclick={oncancel}>Cancel</button>
    <button type="submit" class="primary" disabled={busy}>
      {isEdit ? "Save" : "Create person"}
    </button>
  </div>
</form>

<style>
  .person-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .title {
    font-size: var(--text-xl);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    color: var(--color-ink);
  }

  .check input {
    width: auto;
  }

  .dates {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-3);
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
