// shortcuts.svelte.test.ts — the keyboard-shortcut registry. The
// api-pulling store deps are mocked so only the pure matcher + the real `modal`
// and `ui` stores are exercised: combo matching, the modal guard, the input-focus
// guard (a bare key never fires while typing; a mod-combo still does), the
// Tree-scope gate, and the declaration-order grouping.

import { afterEach, beforeEach, expect, test, vi } from "vitest";

vi.mock("./stores/undo.svelte", () => ({ undo: { runUndo: vi.fn() } }));
vi.mock("./stores/search.svelte", () => ({ searchPalette: { open: vi.fn() } }));
vi.mock("./stores/selection.svelte", () => ({
  selection: { startCreate: vi.fn() },
}));

import {
  SHORTCUTS,
  groupedShortcuts,
  matchShortcut,
} from "./shortcuts.svelte";
import { modal } from "./stores/modal.svelte";
import { ui } from "./stores/ui.svelte";

function keydown(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent("keydown", init);
}

beforeEach(() => {
  ui.showLibrary();
  document.body.innerHTML = "";
});

afterEach(() => {
  // Leave focus + the body clean for the next test.
  document.body.innerHTML = "";
});

test("Ctrl or Cmd + K matches the jump-to-person shortcut", () => {
  expect(matchShortcut(keydown({ key: "k", ctrlKey: true }))?.combo).toBe("mod+k");
  expect(matchShortcut(keydown({ key: "k", metaKey: true }))?.combo).toBe("mod+k");
});

test("the view-switch and undo combos match", () => {
  expect(matchShortcut(keydown({ key: "1", ctrlKey: true }))?.combo).toBe("mod+1");
  expect(matchShortcut(keydown({ key: "z", ctrlKey: true }))?.combo).toBe("mod+z");
  expect(matchShortcut(keydown({ key: "n", ctrlKey: true }))?.combo).toBe("mod+n");
});

test("? matches the help shortcut", () => {
  expect(matchShortcut(keydown({ key: "?" }))?.combo).toBe("?");
});

test("an unbound combo matches nothing", () => {
  expect(matchShortcut(keydown({ key: "q", ctrlKey: true }))).toBeNull();
});

test("no shortcut fires while a modal is open", () => {
  const close = modal.open();
  expect(matchShortcut(keydown({ key: "k", ctrlKey: true }))).toBeNull();
  close();
  expect(matchShortcut(keydown({ key: "k", ctrlKey: true }))?.combo).toBe("mod+k");
});

test("a bare-key shortcut does NOT fire while typing in a field", () => {
  ui.showTree();
  const input = document.createElement("input");
  document.body.appendChild(input);
  input.focus();
  expect(document.activeElement).toBe(input);
  expect(matchShortcut(keydown({ key: "f" }))).toBeNull(); // F suppressed while typing
  input.blur();
  expect(matchShortcut(keydown({ key: "f" }))?.combo).toBe("f"); // fires once focus leaves
});

test("a mod-combo still fires while typing (it is a safe combo)", () => {
  const input = document.createElement("input");
  document.body.appendChild(input);
  input.focus();
  expect(matchShortcut(keydown({ key: "k", ctrlKey: true }))?.combo).toBe("mod+k");
});

test("a Tree-scoped shortcut fires only on the Tree view", () => {
  ui.showLibrary();
  expect(matchShortcut(keydown({ key: "f" }))).toBeNull();
  ui.showTree();
  expect(matchShortcut(keydown({ key: "f" }))?.combo).toBe("f");
});

test("groupedShortcuts buckets every binding in declaration order", () => {
  const groups = groupedShortcuts();
  expect(groups.map((g) => g.name)).toEqual([
    "Navigation",
    "Editing",
    "Tree",
    "Help",
  ]);
  const total = groups.reduce((n, g) => n + g.items.length, 0);
  expect(total).toBe(SHORTCUTS.length);
});
