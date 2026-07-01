// DetailPopover.test.ts — the canvas's read-only person popover. A pure
// presentational component (data + callbacks in), so no store/IPC mock is needed
// (the real `modal` store is dependency-free). These tests pin the summary
// render, the events cap, the portrait toggle, the two navigation callbacks, the
// isRoot gate on "Center chart here", the Esc-closes contract, and the loading
// shell.

import { fireEvent, render } from "@testing-library/svelte";
import { expect, test, vi } from "vitest";

import type { Event, EventKind, Individual, PersonView } from "../lib/types";
import DetailPopover from "./DetailPopover.svelte";

const ev = (id: number, kind: EventKind, year: number | null): Event => ({
  id,
  subject: { Individual: 1 },
  kind,
  date:
    year === null
      ? null
      : { Single: { modifier: "Exact", date: { year, month: null, day: null } } },
  place: null,
  notes: null,
});

interface Over {
  individual?: Partial<Individual>;
  events?: Event[];
  partner_in?: number[];
  child_in?: number[];
}

const view = (over: Over = {}): PersonView => ({
  individual: {
    id: 1,
    given_name: "Ada",
    surname: "Lovelace",
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Female",
    living: false,
    notes: null,
    ...over.individual,
  },
  names: [],
  events: over.events ?? [ev(1, "Birth", 1815), ev(2, "Death", 1852)],
  partner_in: over.partner_in ?? [10],
  child_in: over.child_in ?? [20, 21],
});

const noop = (): void => {};

test("renders the name, lifespan, sex/status, events, family counts, and notes", () => {
  const { getByText, container } = render(DetailPopover, {
    props: {
      view: view({ individual: { notes: "A note about Ada." } }),
      oncenter: noop,
      onopenlibrary: noop,
      onclose: noop,
    },
  });

  expect(getByText("Ada Lovelace")).toBeTruthy();
  expect(getByText("1815–1852")).toBeTruthy();
  expect(getByText("Female")).toBeTruthy();
  expect(getByText("Deceased")).toBeTruthy();
  expect(getByText("Birth")).toBeTruthy();
  expect(getByText("Death")).toBeTruthy();
  expect(getByText("Families: 1 as partner · 2 as child")).toBeTruthy();
  expect(getByText("A note about Ada.")).toBeTruthy();
  // A11y: it is a person-labelled dialog.
  const dialog = container.querySelector('[role="dialog"]');
  expect(dialog?.getAttribute("aria-label")).toBe("Details for Ada Lovelace");
});

test("caps the events list and summarizes the remainder as '+N more'", () => {
  const events = Array.from({ length: 8 }, (_, i) => ev(i + 1, "Residence", 1900 + i));
  const { getByText, container } = render(DetailPopover, {
    props: { view: view({ events }), oncenter: noop, onopenlibrary: noop, onclose: noop },
  });
  // Six events shown + one "+2 more" row.
  expect(container.querySelectorAll("ul.events li")).toHaveLength(7);
  expect(getByText("+2 more")).toBeTruthy();
});

test("shows a portrait <img> when a url is given, and none otherwise", () => {
  const { container: withUrl } = render(DetailPopover, {
    props: {
      view: view(),
      portraitUrl: "asset://face.png",
      oncenter: noop,
      onopenlibrary: noop,
      onclose: noop,
    },
  });
  expect(withUrl.querySelector("img.portrait")?.getAttribute("src")).toBe("asset://face.png");

  const { container: none } = render(DetailPopover, {
    props: { view: view(), oncenter: noop, onopenlibrary: noop, onclose: noop },
  });
  expect(none.querySelector("img.portrait")).toBeNull();
});

test("Center chart here calls oncenter with the person id; Open in Library calls onopenlibrary", async () => {
  const oncenter = vi.fn();
  const onopenlibrary = vi.fn();
  const { getByText } = render(DetailPopover, {
    props: { view: view(), oncenter, onopenlibrary, onclose: noop },
  });
  await fireEvent.click(getByText("Center chart here"));
  expect(oncenter).toHaveBeenCalledWith(1);
  await fireEvent.click(getByText("Open in Library"));
  expect(onopenlibrary).toHaveBeenCalledWith(1);
});

test("hides Center chart here when the person is already the focal root", () => {
  const { queryByText } = render(DetailPopover, {
    props: { view: view(), isRoot: true, oncenter: noop, onopenlibrary: noop, onclose: noop },
  });
  expect(queryByText("Center chart here")).toBeNull();
  expect(queryByText("Open in Library")).toBeTruthy();
});

test("Escape closes the popover", async () => {
  const onclose = vi.fn();
  const { container } = render(DetailPopover, {
    props: { view: view(), oncenter: noop, onopenlibrary: noop, onclose },
  });
  await fireEvent.keyDown(container.querySelector('[role="dialog"]')!, { key: "Escape" });
  expect(onclose).toHaveBeenCalledOnce();
});

test("shows a loading shell while the view is null", () => {
  const { getByText, container } = render(DetailPopover, {
    props: { view: null, oncenter: noop, onopenlibrary: noop, onclose: noop },
  });
  expect(getByText("Loading…")).toBeTruthy();
  expect(container.querySelector("h2.name")).toBeNull();
});
