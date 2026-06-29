// shortcuts.svelte.ts — the keyboard-shortcut registry + the ONE global keydown
// dispatcher. A small data table of bindings, an input-focus guard
// (a bare-key binding never fires while typing), and a modal guard (a dialog or
// palette owns the keyboard while open). The actions only call existing store
// APIs — the registry dispatches; it owns no logic. `matchShortcut` is pure (no
// side effects) so it is unit-testable; `dispatchShortcut` is the window handler.

import { modal } from "./stores/modal.svelte";
import { searchPalette } from "./stores/search.svelte";
import { selection } from "./stores/selection.svelte";
import { ui } from "./stores/ui.svelte";
import { undo } from "./stores/undo.svelte";

// — the Tree fit handle, registered by TreeView when its canvas is ready (the
//   `F` shortcut reuses the same fit the controls-bar button triggers). —
let treeFitHandle: (() => void) | null = null;
export const treeFit = {
  /** TreeView calls this with its canvas fit handle (or `null` when it unmounts). */
  set(fn: (() => void) | null): void {
    treeFitHandle = fn;
  },
  /** Fit the chart to the screen, if a Tree canvas is mounted. */
  run(): void {
    treeFitHandle?.();
  },
};

// — the `?` shortcuts-help overlay open state (the registry opens it; App mounts
//   ShortcutsHelp when it is open). A tiny runes store. —
class ShortcutsHelpState {
  open = $state(false);
  show(): void {
    this.open = true;
  }
  close(): void {
    this.open = false;
  }
}
export const shortcutsHelp = new ShortcutsHelpState();

/** One keyboard binding. `combo` is the normalized form `matchShortcut` produces. */
export interface Shortcut {
  /** Normalized combo: `mod+k`, `mod+1`, `f`, `?`. `mod` = Ctrl or ⌘. */
  combo: string;
  /** Human description (the `?` overlay lists these). */
  label: string;
  /** The reference group the overlay buckets it under. */
  group: string;
  /** `tree` bindings fire only while the Tree view is active. */
  scope: "global" | "tree";
  /** The action — only ever a call into an existing store. */
  run: () => void;
}

/** The shipped binding set. */
export const SHORTCUTS: readonly Shortcut[] = [
  {
    combo: "mod+k",
    label: "Find a person",
    group: "Navigation",
    scope: "global",
    run: () => searchPalette.open(),
  },
  {
    combo: "mod+1",
    label: "Library",
    group: "Navigation",
    scope: "global",
    run: () => ui.showLibrary(),
  },
  {
    combo: "mod+2",
    label: "Tree",
    group: "Navigation",
    scope: "global",
    run: () => ui.showTree(),
  },
  {
    combo: "mod+3",
    label: "Sources",
    group: "Navigation",
    scope: "global",
    run: () => ui.showSources(),
  },
  {
    combo: "mod+n",
    label: "New person",
    group: "Editing",
    scope: "global",
    run: () => {
      ui.showLibrary();
      selection.startCreate("person");
    },
  },
  {
    combo: "mod+z",
    label: "Undo the last delete",
    group: "Editing",
    scope: "global",
    run: () => {
      void undo.runUndo();
    },
  },
  {
    combo: "f",
    label: "Fit the chart to the screen",
    group: "Tree",
    scope: "tree",
    run: () => treeFit.run(),
  },
  {
    combo: "?",
    label: "Show this shortcuts reference",
    group: "Help",
    scope: "global",
    run: () => shortcutsHelp.show(),
  },
];

/** The registry grouped (in declaration order) for the `?` overlay. */
export function groupedShortcuts(): { name: string; items: Shortcut[] }[] {
  const groups: { name: string; items: Shortcut[] }[] = [];
  for (const shortcut of SHORTCUTS) {
    let group = groups.find((g) => g.name === shortcut.group);
    if (group === undefined) {
      group = { name: shortcut.group, items: [] };
      groups.push(group);
    }
    group.items.push(shortcut);
  }
  return groups;
}

/** Normalize a keydown into a `combo` string, or `null` for a bare modifier. */
function normalize(e: KeyboardEvent): string | null {
  const { key } = e;
  if (key === "Control" || key === "Meta" || key === "Shift" || key === "Alt") {
    return null;
  }
  if (key === "?") return "?"; // Shift+/ — no `mod+` prefix
  const base = key.length === 1 ? key.toLowerCase() : key;
  return e.ctrlKey || e.metaKey ? `mod+${base}` : base;
}

/** Whether focus is in a text-entry control (the input-focus guard). */
function isTyping(): boolean {
  const el = typeof document === "undefined" ? null : document.activeElement;
  if (el === null) return false;
  if (
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    el instanceof HTMLSelectElement
  ) {
    return true;
  }
  return el instanceof HTMLElement && el.isContentEditable;
}

/**
 * Match a keydown against the registry, returning the shortcut to run — or `null`
 * if it should not fire: a modal owns the keyboard, the combo is unbound, a
 * bare-key binding while typing, or a Tree-scoped binding off the Tree. Pure.
 */
export function matchShortcut(e: KeyboardEvent): Shortcut | null {
  if (modal.isOpen) return null; // a dialog/palette owns the keyboard
  const combo = normalize(e);
  if (combo === null) return null;
  const isModCombo = combo.startsWith("mod+");
  if (!isModCombo && isTyping()) return null; // bare keys never fire while typing
  const shortcut = SHORTCUTS.find((s) => s.combo === combo);
  if (shortcut === undefined) return null;
  if (shortcut.scope === "tree" && ui.view !== "tree") return null;
  return shortcut;
}

/** The single global keydown handler (mounted on `<svelte:window>` in App). */
export function dispatchShortcut(e: KeyboardEvent): void {
  const shortcut = matchShortcut(e);
  if (shortcut === null) return;
  e.preventDefault();
  shortcut.run();
}
