<script lang="ts">
  // DetailPopover — the canvas's read-only person detail popover. A
  // compact summary of a PersonView (name, lifespan, sex, living/deceased,
  // portrait, capped events, family counts, notes) plus two NAVIGATION actions
  // — Center chart here / Open in Library — anchored beside the clicked card by
  // the host (TreeCanvas). It does NO editing (D2) and NO layout math: the host
  // positions it; this only renders the summary and emits callbacks. It reuses
  // the shared display formatters (no date math in TS) and registers with the
  // modal store while open so the shortcut registry stands down (the popover
  // owns Esc). Renders a "Loading…" shell while the view is still resolving.

  import { tick } from "svelte";

  import { eventKindLabel } from "../lib/drafts";
  import { dateYear, displayName, lifespanYears } from "../lib/format";
  import { modal } from "../lib/stores/modal.svelte";
  import type { PersonView } from "../lib/types";

  interface Props {
    /** The loaded person view, or null while it resolves (the loading shell). */
    view: PersonView | null;
    /** The person's resolved portrait URL (from the canvas's already-batched map), or null. */
    portraitUrl?: string | null;
    /** Hide "Center chart here" when this person is already the chart's focal root. */
    isRoot?: boolean;
    /** Re-root the chart on this person (identical to double-click), then close. */
    oncenter: (personId: number) => void;
    /** Select this person in the Library and switch to it, then close. */
    onopenlibrary: (personId: number) => void;
    /** Close the popover (Esc). */
    onclose: () => void;
  }
  let {
    view,
    portraitUrl = null,
    isRoot = false,
    oncenter,
    onopenlibrary,
    onclose,
  }: Props = $props();

  // Cap the events list so a person with many events can't grow a full-height
  // popover; the remainder is summarized as "+N more".
  const EVENT_CAP = 6;

  const name = $derived(view ? displayName(view.individual) : "");
  const span = $derived(view ? lifespanYears(view.events) : "");
  const shownEvents = $derived(view ? view.events.slice(0, EVENT_CAP) : []);
  const moreEvents = $derived(view ? Math.max(0, view.events.length - EVENT_CAP) : 0);

  let dialog = $state<HTMLDivElement | null>(null);

  // Move focus into the popover on open (the modal owns focus); the host restores
  // focus to the originating node on close.
  $effect(() => {
    void tick().then(() => dialog?.focus());
  });

  // Register with the modal store while mounted so the shortcut registry stands
  // down (the popover owns Esc). Non-blocking — no page-wide backdrop or trap.
  $effect(() => modal.open());

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<div
  class="popover"
  role="dialog"
  aria-label={view ? `Details for ${name}` : "Person details"}
  aria-modal="false"
  tabindex="-1"
  bind:this={dialog}
  onkeydown={onKeydown}
>
  {#if view}
    <header class="head">
      {#if portraitUrl}
        <img class="portrait" src={portraitUrl} alt="" />
      {/if}
      <div class="ident">
        <h2 class="name">{name}</h2>
        <p class="meta">
          {#if span}<span class="span">{span}</span> · {/if}<span>{view.individual.sex}</span>
          · <span>{view.individual.living ? "Living" : "Deceased"}</span>
        </p>
      </div>
    </header>

    {#if shownEvents.length > 0}
      <ul class="events" role="list">
        {#each shownEvents as ev (ev.id)}
          <li>
            <span class="ev-kind">{eventKindLabel(ev.kind)}</span>
            {#if dateYear(ev.date)}<span class="ev-year">{dateYear(ev.date)}</span>{/if}
          </li>
        {/each}
        {#if moreEvents > 0}
          <li class="more">+{moreEvents} more</li>
        {/if}
      </ul>
    {/if}

    <p class="families">
      Families: {view.partner_in.length} as partner · {view.child_in.length} as child
    </p>

    {#if view.individual.notes}
      <p class="notes">{view.individual.notes}</p>
    {/if}

    <div class="actions">
      {#if !isRoot}
        <button type="button" onclick={() => oncenter(view.individual.id)}>
          Center chart here
        </button>
      {/if}
      <button type="button" class="primary" onclick={() => onopenlibrary(view.individual.id)}>
        Open in Library
      </button>
    </div>
  {:else}
    <p class="loading">Loading…</p>
  {/if}
</div>

<style>
  .popover {
    width: min(20rem, 90vw);
    padding: var(--space-4);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-2);
    /* A soft fade/scale-in, neutralized by the global reduced-motion reset. */
    animation: pop var(--motion-fast);
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.98);
    }
  }

  .popover:focus {
    outline: none;
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-3);
  }

  .portrait {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    object-fit: cover;
    border: 1px solid var(--color-hairline);
    flex: none;
  }

  .name {
    font-family: var(--font-serif);
    font-size: var(--text-lg);
    color: var(--color-ink);
    line-height: var(--leading-tight);
  }

  .meta {
    margin-top: 2px;
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .events {
    list-style: none;
    margin: 0 0 var(--space-3);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .events li {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    font-size: var(--text-sm);
  }

  .ev-kind {
    flex: 1;
    color: var(--color-ink);
  }

  .ev-year {
    flex: none;
    color: var(--color-ink-soft);
    font-size: var(--text-xs);
  }

  .more {
    color: var(--color-ink-soft);
    font-size: var(--text-xs);
  }

  .families {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .notes {
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--color-hairline);
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
    white-space: pre-wrap;
    /* Clamp a long note to a few lines so the popover stays compact. */
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .loading {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .actions button {
    padding: var(--space-1) var(--space-3);
    background: transparent;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
    font-size: var(--text-sm);
  }

  .actions button:hover {
    border-color: var(--color-accent);
  }

  .actions button.primary {
    background: var(--color-accent-weak);
    border-color: var(--color-accent);
  }
</style>
