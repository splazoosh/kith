<script lang="ts">
  // FileMenu — the consolidated "File ▾" dropdown in the DatabaseBar. It collects the
  // database-lifecycle and interchange actions that don't need to be one click away
  // (Open / Create / Import GEDCOM / Import LB / Export GEDCOM / Close); folding them
  // here declutters the header so the frequently-used controls (the view switch,
  // Search, About, theme) stay on the bar. Open/Create/Import always work (with or
  // without a database open — Import always makes a new tree); Export and Close show
  // only when a database is open. The dialog plugin (via dbActions/exportActions) only
  // ever returns a path string — all filesystem IO stays in the Rust commands.

  import {
    importGedcom,
    importLb,
    pickAndCreate,
    pickAndOpen,
  } from "../lib/dbActions";
  import { exportGedcom } from "../lib/exportActions";
  import { db } from "../lib/stores/db.svelte";

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);

  function basename(path: string): string {
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  }

  // Seed the GEDCOM-export filename from the open DB's name (sans extension); "tree" if none.
  function exportName(): string {
    return db.current
      ? basename(db.current.path).replace(/\.[^.]+$/, "")
      : "tree";
  }

  function toggle(): void {
    open = !open;
  }

  function close(): void {
    open = false;
  }

  // Collapse the menu, then run the action (its own native dialog takes over).
  function run(action: () => void): void {
    close();
    action();
  }

  // Close on an outside click or Escape while open; Escape returns focus to the trigger.
  function onWindowPointer(e: MouseEvent): void {
    if (open && root && !root.contains(e.target as Node)) close();
  }
  function onWindowKey(e: KeyboardEvent): void {
    if (open && e.key === "Escape") {
      e.preventDefault();
      close();
      trigger?.focus();
    }
  }
</script>

<svelte:window onclick={onWindowPointer} onkeydown={onWindowKey} />

<div class="filemenu" bind:this={root}>
  <button
    type="button"
    class="trigger"
    aria-haspopup="menu"
    aria-expanded={open}
    bind:this={trigger}
    onclick={toggle}
  >
    File <span class="caret" aria-hidden="true">▾</span>
  </button>

  {#if open}
    <div class="menu" role="menu" aria-label="File">
      <button type="button" role="menuitem" onclick={() => run(pickAndOpen)}>
        Open…
      </button>
      <button type="button" role="menuitem" onclick={() => run(pickAndCreate)}>
        Create…
      </button>
      <div class="sep" role="separator"></div>
      <button
        type="button"
        role="menuitem"
        onclick={() => run(() => void importGedcom())}
      >
        Import GEDCOM…
      </button>
      <button
        type="button"
        role="menuitem"
        onclick={() => run(() => void importLb())}
      >
        Import LB (JSON)…
      </button>
      {#if db.current}
        <button
          type="button"
          role="menuitem"
          onclick={() => run(() => void exportGedcom(exportName()))}
        >
          Export GEDCOM…
        </button>
        <div class="sep" role="separator"></div>
        <button
          type="button"
          role="menuitem"
          onclick={() => run(() => void db.close())}
        >
          Close database
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .filemenu {
    position: relative;
    display: inline-flex;
  }

  /* Matches the bar's other ghost buttons (DatabaseBar .actions button). */
  .trigger {
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
    transition: border-color var(--motion-fast);
  }

  .trigger:hover,
  .trigger[aria-expanded="true"] {
    border-color: var(--color-accent);
  }

  .caret {
    font-size: var(--text-xs);
    color: var(--color-ink-soft);
  }

  /* The popover: a token-styled surface dropped under the trigger, right-aligned so
     it never spills off the bar's right edge. */
  .menu {
    position: absolute;
    top: calc(100% + var(--space-1));
    right: 0;
    z-index: 50;
    min-width: 12rem;
    display: flex;
    flex-direction: column;
    padding: var(--space-1);
    background: var(--color-surface);
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-2);
  }

  .menu button {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    text-align: left;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--color-ink);
    white-space: nowrap;
  }

  .menu button:hover,
  .menu button:focus-visible {
    background: var(--color-accent-weak);
  }

  .sep {
    height: 1px;
    margin: var(--space-1) 0;
    background: var(--color-hairline);
  }
</style>
