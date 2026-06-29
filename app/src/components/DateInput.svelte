<script lang="ts">
  // DateInput — the genealogical date field, the one place the date
  // subsystem meets the user. A controlled <input> bound to the RAW string; on
  // input it debounces a `parse_date` round-trip and renders the command's
  // preview. It parses nothing itself: success shows the long form + a
  // modifier chip; a `validation` rejection (a half-typed / unrecognized date)
  // shows a QUIET inline hint and does NOT toast — that channel is for real
  // faults. The raw string is emitted immediately and verbatim, so save never
  // waits on the preview and never loses what the user typed.

  import { onDestroy } from "svelte";

  import * as api from "../lib/api";
  import { asCommandError, isCommandError } from "../lib/errors";
  import { toast } from "../lib/stores/toast.svelte";
  import type { DatePreview } from "../lib/types";

  interface Props {
    /** The raw GEDCOM-style string (bindable). Always exactly what was typed. */
    value: string;
    label: string;
    id: string;
    placeholder?: string;
    disabled?: boolean;
  }
  let {
    value = $bindable(""),
    label,
    id,
    placeholder = "e.g. 12 Mar 1887, ABT 1850, BET 1850 AND 1860",
    disabled = false,
  }: Props = $props();

  const DEBOUNCE_MS = 200;
  const hintId = $derived(`${id}-hint`);

  let preview = $state<DatePreview | null>(null);
  let hint = $state<"idle" | "ok" | "unrecognized">("idle");

  // A monotonic token discards stale `parse_date` responses (and any in-flight
  // call on unmount), so fast typing never flashes an older preview.
  let token = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function onInput(e: Event & { currentTarget: HTMLInputElement }): void {
    const raw = e.currentTarget.value;
    value = raw; // emit upward immediately — the form owns the value.
    schedule(raw);
  }

  function schedule(raw: string): void {
    if (timer !== undefined) clearTimeout(timer);
    if (raw.trim() === "") {
      // Empty is undated: no call, no hint, the value is "".
      preview = null;
      hint = "idle";
      return;
    }
    timer = setTimeout(() => void run(raw), DEBOUNCE_MS);
  }

  async function run(raw: string): Promise<void> {
    const mine = ++token;
    try {
      const p = await api.parseDate(raw);
      if (mine !== token) return; // a newer keystroke superseded this.
      preview = p;
      hint = "ok";
    } catch (e) {
      if (mine !== token) return;
      if (isCommandError(e) && e.kind === "validation") {
        // Expected "still typing / unrecognized" — gentle, local, NOT a toast.
        preview = null;
        hint = "unrecognized";
      } else {
        // A genuine fault (a plugin error, a dropped IPC) — surface it.
        toast.pushError(asCommandError(e));
      }
    }
  }

  onDestroy(() => {
    if (timer !== undefined) clearTimeout(timer);
    token++; // invalidate any in-flight response.
  });
</script>

<div class="date-field">
  <label for={id}>{label}</label>
  <input
    {id}
    type="text"
    {value}
    {placeholder}
    {disabled}
    oninput={onInput}
    aria-describedby={hintId}
    title={preview?.short}
    autocomplete="off"
    spellcheck="false"
  />
  <div id={hintId} class="hint" aria-live="polite">
    {#if hint === "ok" && preview}
      <span class="preview">{preview.long}</span>
      <span class="chip">{preview.modifier}</span>
    {:else if hint === "unrecognized"}
      <span class="unrecognized">unrecognized — will be saved as written</span>
    {/if}
  </div>
</div>

<style>
  .date-field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  label {
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .hint {
    min-height: 1.25rem;
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }

  .preview {
    color: var(--color-ink);
  }

  .chip {
    flex: none;
    font-size: var(--text-xs);
    text-transform: lowercase;
    color: var(--color-accent-text);
    background: var(--color-accent-weak);
    border-radius: 999px;
    padding: 0 var(--space-2);
  }

  .unrecognized {
    color: var(--color-ink-soft);
    font-style: italic;
  }
</style>
