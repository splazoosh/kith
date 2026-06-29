// EventEditor.test.ts — jsdom component test. Mocks `api` and
// proves the open-enum `Other` path: choosing "Other…", typing a code, and
// submitting calls `eventAdd` with `kind: { Other: <code> }`, the context
// subject, and a null date (the empty field is undated, never "").

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({ eventAdd: vi.fn(), parseDate: vi.fn() }));

import EventEditor from "./EventEditor.svelte";
import * as api from "../lib/api";
import type { EventSubject } from "../lib/types";

const eventAdd = vi.mocked(api.eventAdd);

beforeEach(() => {
  eventAdd.mockReset();
  vi.mocked(api.parseDate).mockReset();
});

test("the Other… reveal submits kind { Other: code } with the context subject", async () => {
  eventAdd.mockResolvedValue({
    id: 1,
    subject: { Individual: 7 },
    kind: { Other: "christening" },
    date: null,
    place: null,
    notes: null,
  });
  const subject: EventSubject = { Individual: 7 };
  const onsaved = vi.fn();
  const { getByLabelText, getByRole } = render(EventEditor, {
    props: { subject, onsaved, oncancel: vi.fn() },
  });

  // Choose "Other…" — the free-text input appears — and type the code.
  await fireEvent.change(getByLabelText("Kind"), { target: { value: "Other" } });
  await fireEvent.input(getByLabelText("Other kind"), {
    target: { value: "christening" },
  });
  await fireEvent.click(getByRole("button", { name: "Add event" }));

  expect(eventAdd).toHaveBeenCalledWith({
    subject: { Individual: 7 },
    kind: { Other: "christening" },
    date: null,
    place_id: null,
    place_name: null,
    notes: null,
  });
  expect(onsaved).toHaveBeenCalledOnce();
});
