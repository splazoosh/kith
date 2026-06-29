// selection.svelte.ts — the selected record, its loaded read-only view,
// and the detail pane's mode. Thin: it holds persisted/shared state plus
// a `view | edit` flag and a `creating` flag; the in-progress form draft lives
// inside the form components, never here.
//
// `select*` loads the composite view and resets to view mode; `reselect()`
// quietly re-loads the current view after a sub-editor write (a new event /
// child / name) WITHOUT the loading flash; `startCreate` / `startEdit` /
// `endEdit` drive the form. A load failure clears the selection and toasts.

import * as api from "../api";
import { asCommandError } from "../errors";
import type { FamilyView, PersonView } from "../types";
import { toast } from "./toast.svelte";

export type Selection =
  | { kind: "person"; id: number; view: PersonView }
  | { kind: "family"; id: number; view: FamilyView };

export type Mode = "view" | "edit";

class SelectionStore {
  current = $state<Selection | null>(null);
  loading = $state(false);
  /** `edit` swaps the read-only preview for the entity form (on `current`). */
  mode = $state<Mode>("view");
  /** Non-null ⇒ the detail pane shows an empty create form for this kind. */
  creating = $state<"person" | "family" | null>(null);

  async selectPerson(id: number): Promise<void> {
    this.loading = true;
    this.creating = null;
    this.mode = "view";
    try {
      const view = await api.personGet(id);
      this.current = { kind: "person", id, view };
    } catch (e) {
      this.current = null;
      toast.pushError(asCommandError(e));
    } finally {
      this.loading = false;
    }
  }

  async selectFamily(id: number): Promise<void> {
    this.loading = true;
    this.creating = null;
    this.mode = "view";
    try {
      const view = await api.familyGet(id);
      this.current = { kind: "family", id, view };
    } catch (e) {
      this.current = null;
      toast.pushError(asCommandError(e));
    } finally {
      this.loading = false;
    }
  }

  /** Re-load the current view in place (after a sub-editor write); no flash. */
  async reselect(): Promise<void> {
    const c = this.current;
    if (c === null) return;
    try {
      if (c.kind === "person") {
        const view = await api.personGet(c.id);
        this.current = { kind: "person", id: c.id, view };
      } else {
        const view = await api.familyGet(c.id);
        this.current = { kind: "family", id: c.id, view };
      }
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  /** Show an empty create form for `kind` in the detail pane. */
  startCreate(kind: "person" | "family"): void {
    this.creating = kind;
    this.mode = "view";
  }

  /** Switch the current record's detail to edit mode. */
  startEdit(): void {
    if (this.current !== null) this.mode = "edit";
  }

  /** Leave edit/create, back to the read-only view. */
  endEdit(): void {
    this.mode = "view";
    this.creating = null;
  }

  clear(): void {
    this.current = null;
    this.mode = "view";
    this.creating = null;
  }
}

export const selection = new SelectionStore();
