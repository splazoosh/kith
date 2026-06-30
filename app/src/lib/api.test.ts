// api.test.ts — node-env vitest over a mocked `invoke`. Proves each client fn
// (a) calls the right command name, (b) shapes args with the right camelCase
// keys (the regression guard for the snake_case-param footgun), and
// (c) routes a rejection through `fromInvokeError` into a typed CommandError.

import { beforeEach, expect, test, vi } from "vitest";

const { invoke, convertFileSrc } = vi.hoisted(() => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((p: string) => `asset://localhost/${p}`),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke, convertFileSrc }));

import * as api from "./api";
import { CommandError } from "./errors";
import type { NewIndividual } from "./types";

// NOTE the block body: an expression-body `() => invoke.mockReset()` would
// RETURN the mock, which vitest registers as a teardown hook and then calls —
// re-invoking the (rejecting) mock after each test as an unhandled rejection.
beforeEach(() => {
  invoke.mockReset();
});

test("dbClose invokes the command with no args", async () => {
  invoke.mockResolvedValue(undefined);
  await api.dbClose();
  expect(invoke).toHaveBeenCalledWith("db_close", undefined);
});

test("personGet passes a bare id", async () => {
  invoke.mockResolvedValue({});
  await api.personGet(7);
  expect(invoke).toHaveBeenCalledWith("person_get", { id: 7 });
});

test("search passes the query + a default limit and returns SearchHit[]", async () => {
  invoke.mockResolvedValue([{ individual: { id: 1 }, context: "Doe" }]);
  const hits = await api.search("doe");
  expect(invoke).toHaveBeenCalledWith("search", { query: "doe", limit: 50 });
  expect(hits).toEqual([{ individual: { id: 1 }, context: "Doe" }]);
});

test("search forwards an explicit limit", async () => {
  invoke.mockResolvedValue([]);
  await api.search("doe", 20);
  expect(invoke).toHaveBeenCalledWith("search", { query: "doe", limit: 20 });
});

test("familyAddPartner uses camelCase arg keys", async () => {
  invoke.mockResolvedValue({
    id: 1,
    partner1: 2,
    partner2: 3,
    union_type: "Marriage",
    notes: null,
  });
  await api.familyAddPartner(1, 2);
  expect(invoke).toHaveBeenCalledWith("family_add_partner", {
    familyId: 1,
    personId: 2,
  });
});

test("familyAddChild camelCases keys and forwards relation + optional order", async () => {
  invoke.mockResolvedValue({});
  await api.familyAddChild(4, 5, "Birth");
  expect(invoke).toHaveBeenCalledWith("family_add_child", {
    familyId: 4,
    personId: 5,
    relation: "Birth",
    order: undefined,
  });
});

test("nameList camelCases individualId", async () => {
  invoke.mockResolvedValue([]);
  await api.nameList(9);
  expect(invoke).toHaveBeenCalledWith("name_list", { individualId: 9 });
});

test("personCreate forwards draft + optional raw dates", async () => {
  invoke.mockResolvedValue({ id: 1 });
  const draft: NewIndividual = {
    given_name: "Jane",
    surname: "Doe",
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Female",
    living: false,
    notes: null,
  };
  await api.personCreate(draft, "ABT 1850");
  expect(invoke).toHaveBeenCalledWith("person_create", {
    draft,
    birth: "ABT 1850",
    death: undefined,
  });
});

test("a contract rejection becomes a typed CommandError", async () => {
  invoke.mockRejectedValue({ kind: "not_found", message: "no individual 9" });
  await expect(api.personGet(9)).rejects.toBeInstanceOf(CommandError);
  await expect(api.personGet(9)).rejects.toMatchObject({
    name: "CommandError",
    kind: "not_found",
    message: "no individual 9",
  });
});

test("a non-contract rejection collapses to unexpected", async () => {
  invoke.mockRejectedValue("boom");
  await expect(api.dbCurrent()).rejects.toMatchObject({
    kind: "unexpected",
    message: "boom",
  });
});

test("computeLayout forwards root, mode, generations", async () => {
  invoke.mockResolvedValue({
    mode: "Descendants",
    nodes: [],
    links: [],
    bounds: { x: 0, y: 0, width: 0, height: 0 },
  });
  await api.computeLayout(1, "Descendants", 4);
  expect(invoke).toHaveBeenCalledWith("compute_layout", {
    root: 1,
    mode: "Descendants",
    generations: 4,
  });
});

test("a computeLayout rejection becomes a typed CommandError", async () => {
  invoke.mockRejectedValue({ kind: "not_found", message: "no individual 9" });
  await expect(api.computeLayout(9, "Ancestors", 2)).rejects.toMatchObject({
    name: "CommandError",
    kind: "not_found",
  });
});

test("exportHtml uses camelCase arg keys and resolves void", async () => {
  invoke.mockResolvedValue(null);
  await api.exportHtml(1, "Descendants", 4, "Dark", false, true, "C:/out/tree.html");
  expect(invoke).toHaveBeenCalledWith("export_html", {
    root: 1,
    mode: "Descendants",
    generations: 4,
    theme: "Dark",
    includeLiving: false,
    portraits: true,
    outPath: "C:/out/tree.html",
  });
});

test("exportGedcom uses the outPath arg key and resolves void", async () => {
  invoke.mockResolvedValue(null);
  await api.exportGedcom("C:/out/tree.ged");
  expect(invoke).toHaveBeenCalledWith("export_gedcom", {
    outPath: "C:/out/tree.ged",
  });
});

test("importGedcom passes filePath + dbPath and returns the GedcomImport", async () => {
  const result = {
    db: { path: "C:/out/tree.db", schema_version: 1 },
    summary: {
      individuals: 2,
      families: 1,
      events: 3,
      names: 0,
      places: 1,
      skipped_tags: { SOUR: 2 },
    },
  };
  invoke.mockResolvedValue(result);
  await expect(
    api.importGedcom("C:/in/tree.ged", "C:/out/tree.db"),
  ).resolves.toEqual(result);
  expect(invoke).toHaveBeenCalledWith("import_gedcom", {
    filePath: "C:/in/tree.ged",
    dbPath: "C:/out/tree.db",
  });
});

test("importLb passes filePath + dbPath and returns the LbImport", async () => {
  const result = {
    db: { path: "C:/out/people.db", schema_version: 2 },
    summary: {
      individuals: 8,
      families: 3,
      events: 4,
      names: 0,
      places: 2,
      skipped_tags: {},
    },
  };
  invoke.mockResolvedValue(result);
  await expect(
    api.importLb("C:/in/people.json", "C:/out/people.db"),
  ).resolves.toEqual(result);
  expect(invoke).toHaveBeenCalledWith("import_lb", {
    filePath: "C:/in/people.json",
    dbPath: "C:/out/people.db",
  });
});

test("mediaImport passes the subject, path, and isPrimary (camelCase keys)", async () => {
  invoke.mockResolvedValue({ media: { id: 1, path: "1.png" }, is_primary: true });
  await api.mediaImport({ Individual: 7 }, "C:/pics/face.png", true);
  expect(invoke).toHaveBeenCalledWith("media_import", {
    subject: { Individual: 7 },
    filePath: "C:/pics/face.png",
    isPrimary: true,
  });
});

test("mediaPaths sends the id list and is fed to assetUrl by callers", async () => {
  invoke.mockResolvedValue({ "1": "C:/db.media/1.png", "2": "C:/db.media/2.jpg" });
  const paths = await api.mediaPaths([1, 2]);
  expect(invoke).toHaveBeenCalledWith("media_paths", { ids: [1, 2] });
  expect(paths).toEqual({ "1": "C:/db.media/1.png", "2": "C:/db.media/2.jpg" });
});

test("assetUrl maps a path through convertFileSrc (the asset protocol)", () => {
  expect(api.assetUrl("C:/db.media/1.png")).toBe("asset://localhost/C:/db.media/1.png");
  expect(convertFileSrc).toHaveBeenCalledWith("C:/db.media/1.png");
});

test("mediaSetPrimary and mediaDelete use their command names + keys", async () => {
  invoke.mockResolvedValue(undefined);
  await api.mediaSetPrimary(3, { Family: 2 });
  expect(invoke).toHaveBeenCalledWith("media_set_primary", {
    media: 3,
    subject: { Family: 2 },
  });
  await api.mediaDelete(3);
  expect(invoke).toHaveBeenCalledWith("media_delete", { id: 3 });
});

test("sourceCreate / sourceUpdate forward the draft (snake_case fields)", async () => {
  invoke.mockResolvedValue({ id: 1, title: "Reg" });
  const draft = {
    title: "Reg",
    author: null,
    publication: null,
    repository: null,
    notes: null,
  };
  await api.sourceCreate(draft);
  expect(invoke).toHaveBeenCalledWith("source_create", { source: draft });
  await api.sourceUpdate(1, draft);
  expect(invoke).toHaveBeenCalledWith("source_update", { id: 1, source: draft });
});

test("sourceGet / sourceDelete pass a bare id", async () => {
  invoke.mockResolvedValue({ source: { id: 1 }, citations: [] });
  await api.sourceGet(1);
  expect(invoke).toHaveBeenCalledWith("source_get", { id: 1 });
  invoke.mockResolvedValue(undefined);
  await api.sourceDelete(1);
  expect(invoke).toHaveBeenCalledWith("source_delete", { id: 1 });
});

test("citationAdd forwards the draft; citationsFor passes the subject; citationDelete the id", async () => {
  invoke.mockResolvedValue({ citation: { id: 1 }, source: { id: 1 } });
  const draft = {
    source: 1,
    subject: { Event: 3 } as const,
    page: "p. 4",
    detail: null,
    confidence: "Primary" as const,
  };
  await api.citationAdd(draft);
  expect(invoke).toHaveBeenCalledWith("citation_add", { citation: draft });

  invoke.mockResolvedValue([]);
  await api.citationsFor({ Event: 3 });
  expect(invoke).toHaveBeenCalledWith("citations_for", { subject: { Event: 3 } });

  invoke.mockResolvedValue(undefined);
  await api.citationDelete(7);
  expect(invoke).toHaveBeenCalledWith("citation_delete", { id: 7 });
});
