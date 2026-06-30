<script lang="ts">
  // DatabaseBar — the persistent header: the open path + schema version, the view
  // switch + Search, and the consolidated File menu (Open / Create / Import / Export /
  // Close). The file/interchange actions live in FileMenu to keep the bar uncluttered;
  // the frequently-used controls (view switch, Search, About, theme) stay one click
  // away here. All filesystem IO is the Rust command's (least privilege).

  import { about } from "../lib/stores/about.svelte";
  import { db } from "../lib/stores/db.svelte";
  import { importSummary } from "../lib/stores/importSummary.svelte";
  import { searchPalette } from "../lib/stores/search.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import FileMenu from "./FileMenu.svelte";
  import ImportSummaryDialog from "./ImportSummaryDialog.svelte";
  import ThemeToggle from "./ThemeToggle.svelte";

  function basename(path: string): string {
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  }
</script>

<header class="bar">
  <div class="brand">
    <span class="logo name">Kith</span>
    {#if db.current}
      <span class="dbinfo" title={db.current.path}>
        <span class="file">{basename(db.current.path)}</span>
        <span class="schema">schema v{db.current.schema_version}</span>
      </span>
    {:else}
      <span class="dbinfo muted">No database open</span>
    {/if}
  </div>

  <div class="actions">
    {#if db.current}
      <div class="viewswitch" role="group" aria-label="View">
        <button
          type="button"
          class:active={ui.view === "library"}
          aria-pressed={ui.view === "library"}
          onclick={() => ui.showLibrary()}
        >
          Library
        </button>
        <button
          type="button"
          class:active={ui.view === "tree"}
          aria-pressed={ui.view === "tree"}
          onclick={() => ui.showTree()}
        >
          Tree
        </button>
        <button
          type="button"
          class:active={ui.view === "sources"}
          aria-pressed={ui.view === "sources"}
          onclick={() => ui.showSources()}
        >
          Sources
        </button>
      </div>
      <!-- Jump-to-person: searches the whole tree and selects (re-roots the Tree
           if it's open). Also opens via Ctrl/Cmd+K (the shortcut registry). -->
      <button type="button" onclick={() => searchPalette.open()}>Search…</button>
    {/if}
    <!-- The consolidated file/interchange menu: Open / Create / Import GEDCOM /
         Import LB / Export GEDCOM / Close. Available with or without a DB open. -->
    <FileMenu />
    <!-- About / help — always reachable (product identity, version, shortcuts). -->
    <button type="button" onclick={() => about.open()}>About</button>
    <ThemeToggle />
  </div>
</header>

{#if importSummary.current}
  <ImportSummaryDialog
    summary={importSummary.current}
    onclose={() => importSummary.clear()}
  />
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-4);
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-hairline);
    box-shadow: var(--shadow-1);
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    min-width: 0;
  }

  .logo {
    font-size: var(--text-lg);
    font-weight: 600;
    color: var(--color-ink);
  }

  .dbinfo {
    display: inline-flex;
    align-items: baseline;
    gap: var(--space-3);
    min-width: 0;
    font-size: var(--text-sm);
    color: var(--color-ink-soft);
  }

  .dbinfo.muted {
    font-style: italic;
  }

  .file {
    color: var(--color-ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .schema {
    flex: none;
    font-size: var(--text-xs);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: none;
  }

  .actions button {
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: 1px solid var(--color-hairline);
    border-radius: var(--radius-md);
    color: var(--color-ink);
    transition: border-color var(--motion-fast);
  }

  .actions button:hover {
    border-color: var(--color-accent);
  }

  /* The Library | Tree segmented control — a quiet pair, the active one
     reads with the accent like the chart-mode control in TreeView. */
  .viewswitch {
    display: inline-flex;
    gap: var(--space-1);
    margin-right: var(--space-2);
  }

  .viewswitch button.active {
    color: var(--color-ink);
    border-color: var(--color-accent);
    background: var(--color-accent-weak);
  }
</style>
