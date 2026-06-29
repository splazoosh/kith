<script lang="ts">
  // App — the shell. Loads the global tokens, surfaces the restart-reopen on
  // mount (db.refresh → db_current), and lays out the persistent DatabaseBar over
  // either the no-database EmptyState or the browse split (Library | RecordPreview),
  // with the Toasts overlay.

  import { onMount } from "svelte";

  import "./styles/global.css";

  import AboutModal from "./components/AboutModal.svelte";
  import DatabaseBar from "./components/DatabaseBar.svelte";
  import EmptyState from "./components/EmptyState.svelte";
  import JumpToPerson from "./components/JumpToPerson.svelte";
  import Library from "./components/Library.svelte";
  import RecordPreview from "./components/RecordPreview.svelte";
  import ShortcutsHelp from "./components/ShortcutsHelp.svelte";
  import SourcesView from "./components/SourcesView.svelte";
  import Toasts from "./components/Toasts.svelte";
  import TreeView from "./components/TreeView.svelte";
  import { dispatchShortcut, shortcutsHelp } from "./lib/shortcuts.svelte";
  import { about } from "./lib/stores/about.svelte";
  import { db } from "./lib/stores/db.svelte";
  import { ui } from "./lib/stores/ui.svelte";

  onMount(() => {
    void db.refresh();
  });
</script>

<!-- The ONE global keyboard-shortcut dispatcher (input-/modal-guarded inside). -->
<svelte:window onkeydown={dispatchShortcut} />

<div class="app">
  <DatabaseBar />
  <main
    class="content"
    class:single={db.current === null || ui.view === "tree" || ui.view === "sources"}
  >
    {#if db.current === null}
      <EmptyState mode="no-db" />
    {:else if ui.view === "tree"}
      <!-- The tree-view swap (the Library↔Tree↔Sources switch lives in the DatabaseBar). -->
      <TreeView />
    {:else if ui.view === "sources"}
      <SourcesView />
    {:else}
      <Library />
      <RecordPreview />
    {/if}
  </main>
  <Toasts />
  <JumpToPerson />
  {#if shortcutsHelp.open}
    <ShortcutsHelp />
  {/if}
  {#if about.isOpen}
    <AboutModal onclose={() => about.close()} />
  {/if}
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .content {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(300px, 1fr) minmax(360px, 1.5fr);
  }

  /* The empty state spans the whole area, not just the left column. */
  .content.single {
    grid-template-columns: 1fr;
  }
</style>
