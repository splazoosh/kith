<script lang="ts">
  // EmptyState — the "nothing to show" prompts: no database open (offer
  // open/create), an open-but-empty database (offer the in-app "Add person"), or
  // the tree view with no root chosen yet (embed the PersonPicker so a person can
  // be picked without bouncing back to the Library).

  import { importGedcom, pickAndCreate, pickAndOpen } from "../lib/dbActions";
  import { chart } from "../lib/stores/chart.svelte";
  import { selection } from "../lib/stores/selection.svelte";
  import PersonPicker from "./PersonPicker.svelte";

  interface Props {
    mode: "no-db" | "no-people" | "tree-no-root";
  }
  let { mode }: Props = $props();
</script>

<div class="empty">
  {#if mode === "no-db"}
    <p class="wordmark">Kith</p>
    <h2>No database open</h2>
    <p>
      Open an existing Kith database, create a new one, or import a GEDCOM to start a
      new tree.
    </p>
    <p class="privacy">
      Your family tree stays on this machine — no account, no server, no telemetry.
    </p>
    <div class="actions">
      <button type="button" class="primary" onclick={pickAndCreate}>
        Create database…
      </button>
      <button type="button" onclick={pickAndOpen}>Open database…</button>
      <button type="button" onclick={() => importGedcom()}>Import GEDCOM…</button>
    </div>
  {:else if mode === "no-people"}
    <h2>No people yet</h2>
    <p>This database is empty. Add the first person to begin building the tree.</p>
    <div class="actions">
      <button
        type="button"
        class="primary"
        onclick={() => selection.startCreate("person")}
      >
        Add person…
      </button>
    </div>
  {:else}
    <h2>Pick a person</h2>
    <p>Choose someone to view their family tree.</p>
    <div class="picker-wrap">
      <PersonPicker onpick={(p) => chart.view(p.id)} placeholder="Search people…" />
    </div>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    height: 100%;
    padding: var(--space-6);
    text-align: center;
    color: var(--color-ink-soft);
  }

  /* The brand mark — a large tinted wordmark (the tokens reserve --color-accent
     for "large marks"); the first thing on the no-database welcome. */
  .wordmark {
    margin: 0;
    font-size: 2.75rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--color-accent);
    max-width: none;
  }

  h2 {
    color: var(--color-ink);
    font-size: var(--text-xl);
  }

  p {
    margin: 0;
    max-width: 32rem;
  }

  /* The offline/privacy reassurance — the no-account/no-telemetry
     promise stated up front on first run. */
  .privacy {
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .actions {
    display: flex;
    gap: var(--space-3);
  }

  /* Keep the embedded picker narrow so the no-root state stays centered. */
  .picker-wrap {
    width: 100%;
    max-width: 22rem;
    text-align: left;
  }

  button {
    padding: var(--space-2) var(--space-4);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    transition: border-color var(--motion-fast);
  }

  button:hover {
    border-color: var(--color-accent);
  }

  button.primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: var(--color-surface);
  }
</style>
