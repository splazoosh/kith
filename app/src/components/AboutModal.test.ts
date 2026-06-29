// AboutModal.test.ts — the About/Help modal. The Tauri core is mocked so the
// real api wrapper + shortcut registry load; this asserts the modal renders the
// version from `about_info`, that Esc closes it, and that the "Keyboard
// shortcuts" button hands off to the ShortcutsHelp overlay.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

const ABOUT = {
  name: "Kith",
  version: "1.0.0",
  identifier: "net.splazoosh.kith",
  license: "MIT",
  repository: "",
  authors: "Kith contributors",
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => (cmd === "about_info" ? ABOUT : undefined)),
  convertFileSrc: vi.fn((p: string) => p),
}));

import { shortcutsHelp } from "../lib/shortcuts.svelte";
import AboutModal from "./AboutModal.svelte";

beforeEach(() => {
  shortcutsHelp.open = false;
});

test("renders the product name, version, and license from about_info", async () => {
  const onclose = vi.fn();
  const { findByText, getByText, queryByText } = render(AboutModal, { props: { onclose } });

  // The version is read from the binary (the one source of truth).
  expect(await findByText("Version 1.0.0")).toBeTruthy();
  expect(getByText("MIT")).toBeTruthy();
  // No repository is set, so the Repository row is hidden.
  expect(queryByText("Repository")).toBeNull();
});

test("Escape closes the modal", async () => {
  const onclose = vi.fn();
  render(AboutModal, { props: { onclose } });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onclose).toHaveBeenCalled();
});

test("the Keyboard shortcuts button closes About and opens the reference", async () => {
  const onclose = vi.fn();
  const { getByText } = render(AboutModal, { props: { onclose } });
  await fireEvent.click(getByText("Keyboard shortcuts"));
  expect(onclose).toHaveBeenCalled();
  expect(shortcutsHelp.open).toBe(true);
});
