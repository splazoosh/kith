// ShortcutsHelp.test.ts — the `?` shortcuts-reference overlay. The
// Tauri core is mocked so the real shortcut registry module loads; this asserts
// the overlay renders the registry's bindings and that Esc / the Close button
// dismiss it.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((p: string) => p),
}));

import { shortcutsHelp } from "../lib/shortcuts.svelte";
import ShortcutsHelp from "./ShortcutsHelp.svelte";

beforeEach(() => {
  shortcutsHelp.open = true;
});

test("renders the binding groups and lists the shortcuts", () => {
  const { getByText, container } = render(ShortcutsHelp);
  expect(getByText("Keyboard shortcuts")).toBeTruthy();
  expect(getByText("Find a person")).toBeTruthy();
  expect(getByText("Undo the last delete")).toBeTruthy();
  expect(getByText("Fit the chart to the screen")).toBeTruthy();
  // The Esc-closes-a-dialog reference row is present.
  expect(getByText("Close the open dialog or palette")).toBeTruthy();
  expect(container.querySelectorAll("kbd").length).toBeGreaterThan(0);
});

test("Escape closes the overlay", async () => {
  render(ShortcutsHelp);
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(shortcutsHelp.open).toBe(false);
});

test("the Close button closes the overlay", async () => {
  const { getByText } = render(ShortcutsHelp);
  await fireEvent.click(getByText("Close"));
  expect(shortcutsHelp.open).toBe(false);
});
