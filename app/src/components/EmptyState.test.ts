// EmptyState.test.ts — the no-database first-run welcome. The
// Tauri core is mocked so the transitive api/dbActions imports load; this asserts
// the no-db mode carries the "Kith" wordmark + the offline/privacy promise above
// the still-present create/open/import affordances.

import { render } from "@testing-library/svelte";
import { expect, test, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((p: string) => p),
}));

import EmptyState from "./EmptyState.svelte";

test("the no-database welcome shows the Kith wordmark and the privacy promise", () => {
  const { getByText } = render(EmptyState, { props: { mode: "no-db" } });

  // The brand wordmark (its own element — exact text match, not the description's
  // "…an existing Kith database…").
  expect(getByText("Kith")).toBeTruthy();
  expect(getByText(/no account, no server, no telemetry/i)).toBeTruthy();

  // The create / open / import affordances remain.
  expect(getByText("Create database…")).toBeTruthy();
  expect(getByText("Open database…")).toBeTruthy();
  expect(getByText("Import GEDCOM…")).toBeTruthy();
});
