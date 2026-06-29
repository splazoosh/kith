// CitationsEditor.test.ts — jsdom test for the citations flow. The api
// is mocked and the shared sources store is seeded; this asserts the FLOW (load →
// list, attach a citation from the source picker, remove), not pixels.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  citationsFor: vi.fn(),
  citationAdd: vi.fn(),
  citationDelete: vi.fn(),
}));

import * as api from "../lib/api";
import { sources } from "../lib/stores/sources.svelte";
import { toast } from "../lib/stores/toast.svelte";
import type { CitationItem, Source } from "../lib/types";
import CitationsEditor from "./CitationsEditor.svelte";

const citationsFor = vi.mocked(api.citationsFor);
const citationAdd = vi.mocked(api.citationAdd);
const citationDelete = vi.mocked(api.citationDelete);
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function source(id: number, title: string): Source {
  return { id, title, author: null, publication: null, repository: null, notes: null };
}

function citation(id: number): CitationItem {
  return {
    citation: {
      id,
      source: 1,
      subject: { Event: 5 },
      page: "p. 1",
      detail: null,
      confidence: "Primary",
    },
    source: source(1, "Register"),
  };
}

beforeEach(() => {
  citationsFor.mockReset().mockResolvedValue([]);
  citationAdd.mockReset();
  citationDelete.mockReset().mockResolvedValue(undefined);
  sources.all = [source(1, "Register"), source(2, "Census")];
  toast.items = [];
});

test("loads and lists a subject's citations with their resolved source", async () => {
  citationsFor.mockResolvedValue([citation(10)]);

  const { getByText } = render(CitationsEditor, {
    props: { subject: { Event: 5 } },
  });
  await flush();

  expect(citationsFor).toHaveBeenCalledWith({ Event: 5 });
  expect(getByText("Register")).toBeTruthy();
});

test("attaching picks a source and posts the citation, then reloads", async () => {
  citationsFor.mockResolvedValue([]);
  citationAdd.mockResolvedValue(citation(11));

  const { getByRole, getByLabelText } = render(CitationsEditor, {
    props: { subject: { Event: 5 } },
  });
  await flush();

  await fireEvent.click(getByRole("button", { name: "+ Add citation" }));
  await fireEvent.input(getByLabelText("Page"), { target: { value: "p. 99" } });
  await fireEvent.change(getByLabelText("Confidence"), {
    target: { value: "Secondary" },
  });
  await fireEvent.click(getByRole("button", { name: "Attach" }));
  await flush();

  expect(citationAdd).toHaveBeenCalledWith({
    source: 1, // the picker defaults to the first source
    subject: { Event: 5 },
    page: "p. 99",
    detail: null,
    confidence: "Secondary",
  });
  expect(citationsFor).toHaveBeenCalledTimes(2); // initial load + reload after attach
});

test("attaching with no sources prompts to add one first", async () => {
  sources.all = [];
  const { getByRole, getByText } = render(CitationsEditor, {
    props: { subject: { Event: 5 } },
  });
  await flush();
  await fireEvent.click(getByRole("button", { name: "+ Add citation" }));

  expect(getByText("Add a source in the Sources view first.")).toBeTruthy();
  expect(citationAdd).not.toHaveBeenCalled();
});

test("Remove deletes the citation and reloads", async () => {
  citationsFor.mockResolvedValue([citation(12)]);

  const { getByRole } = render(CitationsEditor, {
    props: { subject: { Event: 5 } },
  });
  await flush();
  await fireEvent.click(getByRole("button", { name: "Remove citation" }));
  await flush();

  expect(citationDelete).toHaveBeenCalledWith(12);
});
