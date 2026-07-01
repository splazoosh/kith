// nodeDetail.svelte.test.ts — the canvas detail-popover store. `api` and
// `toast` are mocked. The store STORES the PersonView `personGet` returns; these
// tests pin open (sync target + async view), the out-of-order guard (rapid
// clicks across nodes), the re-target, the error path (toast + close), and that
// close invalidates an in-flight open.

import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../api", () => ({ personGet: vi.fn() }));
vi.mock("./toast.svelte", () => ({ toast: { pushError: vi.fn() } }));

import * as api from "../api";
import { CommandError } from "../errors";
import type { PersonView } from "../types";
import { nodeDetail } from "./nodeDetail.svelte";
import { toast } from "./toast.svelte";

const personView = (id: number): PersonView => ({
  individual: {
    id,
    given_name: "Ada",
    surname: "Lovelace",
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Female",
    living: false,
    notes: null,
  },
  names: [],
  events: [],
  partner_in: [],
  child_in: [],
});

function deferred<T>(): { promise: Promise<T>; resolve: (value: T) => void } {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.mocked(api.personGet).mockReset().mockResolvedValue(personView(1));
  vi.mocked(toast.pushError).mockReset();
  nodeDetail.close();
});

test("open loads the PersonView and anchors the target", async () => {
  vi.mocked(api.personGet).mockResolvedValueOnce(personView(1));
  await nodeDetail.open(1);
  expect(api.personGet).toHaveBeenCalledWith(1);
  expect(nodeDetail.target).toEqual({ kind: "person", id: 1 });
  expect(nodeDetail.view?.individual.id).toBe(1);
  expect(nodeDetail.loading).toBe(false);
});

test("open sets the target synchronously (so the popover anchors before the view resolves)", () => {
  const slow = deferred<PersonView>();
  vi.mocked(api.personGet).mockReturnValueOnce(slow.promise);
  void nodeDetail.open(5);
  // target is set immediately (anchor + "Loading…" shell); no stale view shows.
  expect(nodeDetail.target).toEqual({ kind: "person", id: 5 });
  expect(nodeDetail.view).toBeNull();
  expect(nodeDetail.loading).toBe(true);
});

test("an out-of-order older response is dropped", async () => {
  const slow = deferred<PersonView>();
  vi.mocked(api.personGet)
    .mockReturnValueOnce(slow.promise) // first open — resolves LATER
    .mockResolvedValueOnce(personView(2)); // second open — resolves now

  void nodeDetail.open(1);
  await nodeDetail.open(2);
  expect(nodeDetail.view?.individual.id).toBe(2);

  slow.resolve(personView(1)); // the stale open finally resolves…
  await new Promise((r) => setTimeout(r, 0));
  expect(nodeDetail.target).toEqual({ kind: "person", id: 2 }); // …and is dropped
  expect(nodeDetail.view?.individual.id).toBe(2);
});

test("open of a second id re-targets", async () => {
  await nodeDetail.open(1);
  vi.mocked(api.personGet).mockResolvedValueOnce(personView(9));
  await nodeDetail.open(9);
  expect(nodeDetail.target).toEqual({ kind: "person", id: 9 });
  expect(nodeDetail.view?.individual.id).toBe(9);
});

test("a rejected personGet toasts and leaves the popover closed", async () => {
  vi.mocked(api.personGet).mockRejectedValueOnce(new CommandError("not_found", "gone"));
  await nodeDetail.open(3);
  expect(toast.pushError).toHaveBeenCalledOnce();
  expect(nodeDetail.target).toBeNull();
  expect(nodeDetail.view).toBeNull();
  expect(nodeDetail.loading).toBe(false);
});

test("close clears and invalidates an in-flight open", async () => {
  const slow = deferred<PersonView>();
  vi.mocked(api.personGet).mockReturnValueOnce(slow.promise);
  void nodeDetail.open(1);
  nodeDetail.close();
  expect(nodeDetail.target).toBeNull();

  slow.resolve(personView(1)); // the invalidated open resolves…
  await new Promise((r) => setTimeout(r, 0));
  expect(nodeDetail.target).toBeNull(); // …and does not reopen
  expect(nodeDetail.view).toBeNull();
});
