// NamesEditor.test.ts — the validation audit on the alternate-name
// form. `api` is mocked; this asserts the client-side guard surfaces a rejected
// rule INLINE (not as a toast) and skips the round-trip.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  nameList: vi.fn().mockResolvedValue([]),
  nameAdd: vi.fn(),
  nameRemove: vi.fn(),
}));

import * as api from "../lib/api";
import NamesEditor from "./NamesEditor.svelte";

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  vi.mocked(api.nameList).mockReset().mockResolvedValue([]);
  vi.mocked(api.nameAdd).mockReset();
});

test("a blank alternate name shows an inline error and does not call the command", async () => {
  const { getByText, findByRole } = render(NamesEditor, {
    props: { individualId: 1 },
  });
  await flush();

  await fireEvent.click(getByText("+ Add name"));
  await fireEvent.click(getByText("Add")); // submit with every field blank

  const alert = await findByRole("alert");
  expect(alert.textContent).toMatch(/needs a given name or a surname/);
  expect(api.nameAdd).not.toHaveBeenCalled();
});

test("a name with a surname clears the guard and calls the command", async () => {
  vi.mocked(api.nameAdd).mockResolvedValue({
    id: 9,
    individual_id: 1,
    kind: "Aka",
    given_name: null,
    surname: "Byron",
    name_prefix: null,
    name_suffix: null,
    sort_order: 0,
  });
  const { getByText, container } = render(NamesEditor, {
    props: { individualId: 1 },
  });
  await flush();

  await fireEvent.click(getByText("+ Add name"));
  // The form's text inputs are Given / Surname / Prefix / Suffix in order.
  const inputs = container.querySelectorAll<HTMLInputElement>("input[type='text']");
  await fireEvent.input(inputs[1], { target: { value: "Byron" } }); // Surname
  await fireEvent.click(getByText("Add"));
  await flush();

  expect(api.nameAdd).toHaveBeenCalledOnce();
});
