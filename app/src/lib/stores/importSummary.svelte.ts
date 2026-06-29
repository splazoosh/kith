// importSummary.svelte.ts — a one-slot channel for the post-import summary dialog.
//
// A fresh GEDCOM import switches the open database (no-db → open), which unmounts the
// EmptyState that may have triggered it. So the summary can't live on the trigger;
// it lives here, and the single ImportSummaryDialog mounted in the persistent
// DatabaseBar reads it — surviving the view swap regardless of which button started
// the import.

import type { ImportSummary } from "../types";

class ImportSummaryStore {
  /** The summary to show, or `null` when no dialog is open. */
  current = $state<ImportSummary | null>(null);

  /** Surface a completed import's summary. */
  show(summary: ImportSummary): void {
    this.current = summary;
  }

  /** Dismiss the dialog. */
  clear(): void {
    this.current = null;
  }
}

export const importSummary = new ImportSummaryStore();
