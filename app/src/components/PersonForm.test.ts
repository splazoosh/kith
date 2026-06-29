// PersonForm.test.ts — jsdom component test. Mocks `api` and
// asserts the exact arg-shaping the IPC depends on: a filled create submits the
// right `NewIndividual` draft plus the raw birth string, and an empty form
// still submits (no invented required field — the core's permissiveness).

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  personCreate: vi.fn(),
  personUpdate: vi.fn(),
  parseDate: vi.fn(),
}));

import PersonForm from "./PersonForm.svelte";
import * as api from "../lib/api";

const personCreate = vi.mocked(api.personCreate);

beforeEach(() => {
  personCreate.mockReset();
  personCreate.mockResolvedValue({
    id: 1,
    given_name: "Jane",
    surname: "Doe",
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Female",
    living: true,
    notes: null,
  });
  vi.mocked(api.parseDate).mockReset();
});

test("a filled create submits the draft and the raw birth string", async () => {
  const onsaved = vi.fn();
  const { getByLabelText, getByRole } = render(PersonForm, {
    props: { onsaved, oncancel: vi.fn() },
  });

  await fireEvent.input(getByLabelText("Given name"), { target: { value: "Jane" } });
  await fireEvent.input(getByLabelText("Surname"), { target: { value: "Doe" } });
  await fireEvent.change(getByLabelText("Sex"), { target: { value: "Female" } });
  await fireEvent.input(getByLabelText("Birth"), { target: { value: "ABT 1850" } });
  await fireEvent.click(getByRole("button", { name: "Create person" }));

  expect(personCreate).toHaveBeenCalledWith(
    {
      given_name: "Jane",
      surname: "Doe",
      name_prefix: null,
      name_suffix: null,
      nickname: null,
      sex: "Female",
      living: true,
      notes: null,
    },
    "ABT 1850",
    undefined,
  );
  expect(onsaved).toHaveBeenCalledOnce();
});

test("an empty form still submits (no invented required field)", async () => {
  const { getByRole } = render(PersonForm, {
    props: { onsaved: vi.fn(), oncancel: vi.fn() },
  });

  await fireEvent.click(getByRole("button", { name: "Create person" }));

  expect(personCreate).toHaveBeenCalledWith(
    {
      given_name: null,
      surname: null,
      name_prefix: null,
      name_suffix: null,
      nickname: null,
      sex: "Unknown",
      living: true,
      notes: null,
    },
    undefined,
    undefined,
  );
});
