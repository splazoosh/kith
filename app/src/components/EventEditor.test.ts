// EventEditor.test.ts — jsdom component test. Mocks `api` and
// proves (1) the add open-enum `Other` path: choosing "Other…", typing a code,
// and submitting calls `eventAdd` with `kind: { Other: <code> }`, the context
// subject, and a null date (the empty field is undated, never ""); and (2) the
// edit path: an `editId` loads the event, seeds every field (the date via
// `format_date`), and Save calls `eventUpdate` re-using the unchanged place id.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  eventAdd: vi.fn(),
  eventGet: vi.fn(),
  eventUpdate: vi.fn(),
  formatDate: vi.fn(),
  parseDate: vi.fn(),
}));

import EventEditor from "./EventEditor.svelte";
import * as api from "../lib/api";
import type { EventSubject } from "../lib/types";

const eventAdd = vi.mocked(api.eventAdd);

beforeEach(() => {
  eventAdd.mockReset();
  vi.mocked(api.eventGet).mockReset();
  vi.mocked(api.eventUpdate).mockReset();
  vi.mocked(api.formatDate).mockReset();
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

test("edit mode seeds the fields and saves via eventUpdate, re-using the place id", async () => {
  vi.mocked(api.eventGet).mockResolvedValue({
    event: {
      id: 5,
      subject: { Individual: 7 },
      kind: "Birth",
      date: {
        Single: { modifier: "About", date: { year: 1850, month: null, day: null } },
      },
      place: 3,
      notes: "born at home",
    },
    place: { id: 3, name: "Bergen", latitude: null, longitude: null, parent: null },
    citations: [],
  });
  // The stored date is turned back into an editable string by the core.
  vi.mocked(api.formatDate).mockResolvedValue("about 1850");
  vi.mocked(api.eventUpdate).mockResolvedValue({
    id: 5,
    subject: { Individual: 7 },
    kind: "Birth",
    date: {
      Single: { modifier: "About", date: { year: 1850, month: null, day: null } },
    },
    place: 3,
    notes: "born at home",
  });
  const onsaved = vi.fn();
  const { getByLabelText, findByRole } = render(EventEditor, {
    props: { subject: { Individual: 7 }, editId: 5, onsaved, oncancel: vi.fn() },
  });

  // The form appears once the async load resolves; every field is seeded.
  const save = await findByRole("button", { name: "Save changes" });
  expect((getByLabelText("Kind") as HTMLSelectElement).value).toBe("Birth");
  expect((getByLabelText("Date") as HTMLInputElement).value).toBe("about 1850");
  expect((getByLabelText("Place") as HTMLInputElement).value).toBe("Bergen");
  expect((getByLabelText("Notes") as HTMLTextAreaElement).value).toBe("born at home");

  await fireEvent.click(save);

  // The place was not touched, so its existing id rides along (no new place row).
  expect(api.eventUpdate).toHaveBeenCalledWith({
    id: 5,
    kind: "Birth",
    date: "about 1850",
    place_id: 3,
    place_name: null,
    notes: "born at home",
  });
  expect(api.eventAdd).not.toHaveBeenCalled();
  expect(onsaved).toHaveBeenCalledOnce();
});
