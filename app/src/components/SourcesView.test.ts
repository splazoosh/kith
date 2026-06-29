// SourcesView.test.ts — jsdom test for the Sources management surface.
// The api is mocked and the shared sources store is seeded; this asserts the FLOW
// (list, open the create form, and delete-with-cascade-confirm), not pixels.

import { fireEvent, render, within } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  sourceList: vi.fn(),
  sourceGet: vi.fn(),
  sourceCreate: vi.fn(),
  sourceUpdate: vi.fn(),
  sourceDelete: vi.fn(),
}));

import * as api from "../lib/api";
import { sources } from "../lib/stores/sources.svelte";
import { toast } from "../lib/stores/toast.svelte";
import type { Source } from "../lib/types";
import SourcesView from "./SourcesView.svelte";

const sourceGet = vi.mocked(api.sourceGet);
const sourceDelete = vi.mocked(api.sourceDelete);
const sourceList = vi.mocked(api.sourceList);
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function source(id: number, title: string): Source {
  return { id, title, author: null, publication: null, repository: null, notes: null };
}

beforeEach(() => {
  sourceGet.mockReset();
  sourceDelete.mockReset().mockResolvedValue(undefined);
  sourceList.mockReset().mockResolvedValue([]);
  sources.all = [source(1, "Bergen Parish Register")];
  toast.items = [];
});

test("lists the source catalogue", () => {
  const { getByText } = render(SourcesView);
  expect(getByText("Bergen Parish Register")).toBeTruthy();
});

test("New source opens the create form", async () => {
  const { getByRole, getByText } = render(SourcesView);
  await fireEvent.click(getByRole("button", { name: "+ New source" }));
  expect(getByText("New source")).toBeTruthy(); // the SourceForm heading
});

test("Delete confirms with the exact cascade count, then deletes", async () => {
  // The source supports two citations — the confirm must name them.
  sourceGet.mockResolvedValue({
    source: source(1, "Bergen Parish Register"),
    citations: [
      {
        id: 1,
        source: 1,
        subject: { Event: 9 },
        page: null,
        detail: null,
        confidence: null,
      },
      {
        id: 2,
        source: 1,
        subject: { Individual: 3 },
        page: null,
        detail: null,
        confidence: null,
      },
    ],
  });

  const { getByRole } = render(SourcesView);
  await fireEvent.click(getByRole("button", { name: "Delete" }));
  await flush();

  const dialog = getByRole("alertdialog");
  expect(within(dialog).getByText(/2 citations/)).toBeTruthy();

  await fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
  await flush();

  expect(sourceDelete).toHaveBeenCalledWith(1);
  expect(sourceList).toHaveBeenCalled(); // reload after the delete
});
