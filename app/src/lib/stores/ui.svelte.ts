// ui.svelte.ts — the top-level view selection. A PURE
// library|tree|sources flag the DatabaseBar segmented control and the App shell
// read. The root is seeded on the chart store directly (chart.view), so this
// store no longer imports chart — it only flips the view. `sources` is the
// Sources management surface.
class UiStore {
  view = $state<"library" | "tree" | "sources">("library");

  showTree(): void {
    this.view = "tree";
  }

  showLibrary(): void {
    this.view = "library";
  }

  showSources(): void {
    this.view = "sources";
  }
}

export const ui = new UiStore();
