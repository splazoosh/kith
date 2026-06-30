// api.ts — the ONE typed wrapper per Tauri command, and the ONLY module that
// imports `invoke`. Components and stores call these functions; they never
// `invoke` directly and never see a raw rejection.
//
// Two-level naming convention, encoded here so no caller improvises a key:
//   • Top-level `invoke` arg KEYS are camelCase (`familyId`, `individualId`) —
//     Tauri v2 maps them to the snake_case Rust command PARAMETERS.
//   • Record / draft / request FIELDS inside a payload stay snake_case —
//     they are plain serde structs, not parameters, and are untouched by the
//     param mapping.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";

import { fromInvokeError } from "./errors";
import type {
  AboutInfo,
  ChartMode,
  ChildLink,
  ChildRelation,
  CitationId,
  CitationItem,
  CitationSubject,
  DatePreview,
  DbInfo,
  Event,
  EventView,
  Family,
  FamilyView,
  GedcomImport,
  Individual,
  LayoutModel,
  LbImport,
  MediaId,
  MediaItem,
  MediaSubject,
  Name,
  NewCitation,
  NewEventRequest,
  NewFamily,
  NewIndividual,
  NewName,
  NewSource,
  PersonView,
  SearchHit,
  Source,
  SourceId,
  SourceView,
  Theme,
  UndoOutcome,
  UpdateEventRequest,
} from "./types";

/** The single translation point: every rejection becomes a `CommandError`. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw fromInvokeError(e);
  }
}

// — database lifecycle —
export const dbCreate = (path: string) => call<DbInfo>("db_create", { path });
export const dbOpen = (path: string) => call<DbInfo>("db_open", { path });
export const dbClose = () => call<void>("db_close");
export const dbCurrent = () => call<DbInfo | null>("db_current");

// — people —
export const personList = () => call<Individual[]>("person_list");
export const personGet = (id: number) => call<PersonView>("person_get", { id });
export const personCreate = (
  draft: NewIndividual,
  birth?: string,
  death?: string,
) => call<Individual>("person_create", { draft, birth, death });
export const personUpdate = (record: Individual) =>
  call<Individual>("person_update", { record });
export const personDelete = (id: number) =>
  call<void>("person_delete", { id });
/** Ranked, multi-field full-text search (names/alt-names/nickname/notes/places).
 *  Returns SearchHit[] best-match-first; `limit` caps the result. */
export const search = (query: string, limit = 50) =>
  call<SearchHit[]>("search", { query, limit });

// — families (camelCase arg keys → snake_case params) —
export const familyList = () => call<Family[]>("family_list");
export const familyGet = (id: number) => call<FamilyView>("family_get", { id });
export const familyCreate = (draft: NewFamily) =>
  call<Family>("family_create", { draft });
export const familyUpdate = (record: Family) =>
  call<Family>("family_update", { record });
export const familyDelete = (id: number) =>
  call<void>("family_delete", { id });
export const familyAddPartner = (familyId: number, personId: number) =>
  call<Family>("family_add_partner", { familyId, personId });
export const familyAddChild = (
  familyId: number,
  personId: number,
  relation: ChildRelation,
  order?: number,
) => call<ChildLink>("family_add_child", { familyId, personId, relation, order });
export const familyRemoveChild = (familyId: number, personId: number) =>
  call<void>("family_remove_child", { familyId, personId });

// — events / names / date (wired now; consumed by the write forms) —
export const eventAdd = (request: NewEventRequest) =>
  call<Event>("event_add", { request });
export const eventGet = (id: number) => call<EventView>("event_get", { id });
export const eventUpdate = (request: UpdateEventRequest) =>
  call<Event>("event_update", { request });
export const eventDelete = (id: number) =>
  call<void>("event_delete", { id });
export const nameAdd = (draft: NewName) => call<Name>("name_add", { draft });
export const nameList = (individualId: number) =>
  call<Name[]>("name_list", { individualId });
export const nameRemove = (id: number) => call<void>("name_remove", { id });
export const parseDate = (input: string) =>
  call<DatePreview>("parse_date", { input });

// — layout (the positioned chart; the canvas renders it, the controls drive it).
//   root/mode/generations are single-token params, so the camelCase arg keys
//   coincide with the snake_case Rust params (here a no-op). —
export const computeLayout = (
  root: number,
  mode: ChartMode,
  generations: number,
) => call<LayoutModel>("compute_layout", { root, mode, generations });

// — export: the GUI surface over kith_core::render::html. The Rust
//   command writes the file (the save dialog supplied the path); the camelCase
//   keys `includeLiving`/`outPath` map to the snake_case command params. —
/** Render the chart rooted at `root` to a single self-contained `.html` file at `outPath`.
 *  The Rust command writes the file (the save dialog supplied the path); resolves on success. */
export const exportHtml = (
  root: number,
  mode: ChartMode,
  generations: number,
  theme: Theme,
  includeLiving: boolean,
  portraits: boolean,
  outPath: string,
) =>
  call<void>("export_html", {
    root,
    mode,
    generations,
    theme,
    includeLiving,
    portraits,
    outPath,
  });

// — GEDCOM interop: the GUI surface over kith_core::gedcom. The Rust
//   commands do the file IO (the dialogs supplied the path strings); the camelCase
//   keys `outPath`/`filePath` map to the snake_case command params. The
//   returned ImportSummary, by contrast, is a serde struct with snake_case keys. —
/** Serialize the whole open database to a GEDCOM 5.5.1 file at `outPath`. The Rust
 *  command writes the file (the save dialog supplied the path); resolves on success. */
export const exportGedcom = (outPath: string) =>
  call<void>("export_gedcom", { outPath });

/** Import a GEDCOM 5.5.1 file into a NEW database at `dbPath` and open it, returning
 *  the new DbInfo + the summary (counts + skipped tags). The Rust command reads the
 *  GEDCOM, creates the database, and attaches it (both dialogs supplied the paths). */
export const importGedcom = (filePath: string, dbPath: string) =>
  call<GedcomImport>("import_gedcom", { filePath, dbPath });

/** Import an "LB" JSON file into a NEW database at `dbPath` and open it, returning
 *  the new DbInfo + the summary. Mirrors `importGedcom` (a fresh-tree import); the
 *  Rust command reads the JSON, creates the database, and attaches it (both dialogs
 *  supplied the paths). */
export const importLb = (filePath: string, dbPath: string) =>
  call<LbImport>("import_lb", { filePath, dbPath });

// — media / portraits: the GUI surface over kith_core::db media CRUD.
//   `media_import` copies the picked file (the open dialog supplied `filePath`);
//   `media_paths` returns absolute paths the canvas feeds to `convertFileSrc`
//   (the asset-protocol display path). camelCase arg keys → snake_case params. —
/** Import an image for `subject`, copying it into the media folder; returns the new item. */
export const mediaImport = (
  subject: MediaSubject,
  filePath: string,
  isPrimary: boolean,
) => call<MediaItem>("media_import", { subject, filePath, isPrimary });

/** List a subject's media (primary first), for the detail-view gallery. */
export const mediaFor = (subject: MediaSubject) =>
  call<MediaItem[]>("media_for", { subject });

/** Resolve media ids to absolute file paths (for `convertFileSrc`). Keys are stringified ids. */
export const mediaPaths = (ids: MediaId[]) =>
  call<Record<MediaId, string>>("media_paths", { ids });

/** Make `media` the subject's primary (portrait). */
export const mediaSetPrimary = (media: MediaId, subject: MediaSubject) =>
  call<void>("media_set_primary", { media, subject });

/** Delete a media row (its links cascade). */
export const mediaDelete = (id: MediaId) => call<void>("media_delete", { id });

// — sources & citations: the GUI surface over kith_core::db source/
//   citation CRUD. Pure DB — no dialog, no path string, no `fs`/asset ACL. The
//   `source`/`citation` payloads are serde structs (snake_case fields); the single
//   `id`/`subject` args coincide with their snake_case params. —
/** Create a source, returning the persisted record. */
export const sourceCreate = (source: NewSource) =>
  call<Source>("source_create", { source });

/** List every source (ascending id). */
export const sourceList = () => call<Source[]>("source_list");

/** Load a source with the facts it supports. */
export const sourceGet = (id: SourceId) =>
  call<SourceView>("source_get", { id });

/** Update a source's fields, returning the updated record. */
export const sourceUpdate = (id: SourceId, source: NewSource) =>
  call<Source>("source_update", { id, source });

/** Delete a source (its citations cascade). */
export const sourceDelete = (id: SourceId) =>
  call<void>("source_delete", { id });

/** Attach a citation linking a source to a fact; returns the item with its source resolved. */
export const citationAdd = (citation: NewCitation) =>
  call<CitationItem>("citation_add", { citation });

/** List a subject's citations, each with its source resolved (no N+1). */
export const citationsFor = (subject: CitationSubject) =>
  call<CitationItem[]>("citations_for", { subject });

/** Delete a citation. */
export const citationDelete = (id: CitationId) =>
  call<void>("citation_delete", { id });

// — undo: pop + restore the last destructive action. The session
//   stack lives on AppState; the restore (with original ids + cascade set) lives
//   in kith_core — this is a thin wrapper over the one `undo` command. —
/** Undo the last delete; resolves to the outcome (kind/label/remaining), or `null`
 *  if there was nothing to undo. */
export const undo = () => call<UndoOutcome | null>("undo");

// — app metadata: the app's identity for the About/Help modal. An
//   app-defined, infallible command (no DB, no ACL); this is its one invoke site. —
/** The product name, version, identifier, license, repository, and authors. */
export const aboutInfo = () => call<AboutInfo>("about_info");

/** Map an absolute media-file path to an asset-protocol URL the WebView can load.
 *  A pure URL transform — no IPC — kept here so `api.ts` stays
 *  the only module importing from `@tauri-apps/api/core`. */
export const assetUrl = (path: string): string => convertFileSrc(path);
