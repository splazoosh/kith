// types.ts — the TypeScript mirror of the `kith-core` serde wire shapes.
//
// This is a typed contract, not a place for logic. The shapes are dictated by
// the Rust serde derives the CLI and Tauri commands share, so the rules below
// are LOAD-BEARING — do not "tidy" them into idiomatic camelCase:
//
//   • Ids (`#[serde(transparent)]` newtypes over i64) → bare `number`. Aliased
//     for readability only; the wire cannot enforce the distinction.
//   • Closed enums → variant-name string unions ("Female", "Birth", "Marriage").
//   • `EventKind` is open → its catch-all arm is `{ Other: string }`.
//   • `EventSubject` / `GenealogicalDate` are externally tagged enums →
//     `{ Individual: number }`, `{ Single: { … } }`, etc.
//   • Record / draft / view FIELDS stay snake_case — they are the same
//     types the CLI serializes to its stable `--json`; renaming would break it.
//   • `Option<T>` → `T | null`, ALWAYS PRESENT — the core derives no
//     `skip_serializing_if`, so an absent value is `null`, never an omitted key.
//     Use `| null`, never `?`.

// — ids (bare numbers; aliases document intent) —
export type PersonId = number;
export type FamilyId = number;
export type EventId = number;
export type NameId = number;
export type PlaceId = number;
export type MediaId = number;
export type SourceId = number;
export type CitationId = number;

// — closed enums (variant-name strings) —
export type Sex = "Male" | "Female" | "Other" | "Unknown";
export type ChildRelation = "Birth" | "Adopted" | "Step" | "Foster";
export type UnionType = "Marriage" | "Partnership" | "Unknown";
export type NameKind = "Birth" | "Married" | "Aka" | "Religious";
// A citation's confidence in its evidence. Maps to GEDCOM QUAY.
export type Confidence = "Primary" | "Secondary" | "Questionable";
export type DateModifier =
  | "Exact"
  | "About"
  | "Before"
  | "After"
  | "Between"
  | "Estimated"
  | "Calculated";

// — open enum: known kinds by name, anything else as { Other: code } —
export type EventKind =
  | "Birth"
  | "Death"
  | "Marriage"
  | "Divorce"
  | "Baptism"
  | "Burial"
  | "Residence"
  | "Occupation"
  | { Other: string };

// — externally-tagged unions —
export type EventSubject = { Individual: PersonId } | { Family: FamilyId };
// MediaSubject mirrors the three nullable FKs on `media_links`.
export type MediaSubject =
  | { Individual: PersonId }
  | { Family: FamilyId }
  | { Event: EventId };
// CitationSubject mirrors the three nullable fact FKs on `citations`.
// The GUI authors only event citations (events-only); the other arms arrive
// via GEDCOM import / the CLI and still display.
export type CitationSubject =
  | { Individual: PersonId }
  | { Family: FamilyId }
  | { Event: EventId };

export interface PartialDate {
  year: number;
  month: number | null;
  day: number | null;
}

export type GenealogicalDate =
  | { Single: { modifier: DateModifier; date: PartialDate } }
  | { Range: { from: PartialDate; to: PartialDate } };

// — records (snake_case fields, mirrors kith_core::model) —
export interface Individual {
  id: PersonId;
  given_name: string | null;
  surname: string | null;
  name_prefix: string | null;
  name_suffix: string | null;
  nickname: string | null;
  sex: Sex;
  living: boolean;
  notes: string | null;
}

export interface Name {
  id: NameId;
  individual_id: PersonId;
  kind: NameKind;
  given_name: string | null;
  surname: string | null;
  name_prefix: string | null;
  name_suffix: string | null;
  sort_order: number;
}

export interface Family {
  id: FamilyId;
  partner1: PersonId | null;
  partner2: PersonId | null;
  union_type: UnionType;
  notes: string | null;
}

export interface ChildLink {
  family_id: FamilyId;
  child_id: PersonId;
  relation: ChildRelation;
  sort_order: number;
}

export interface Event {
  id: EventId;
  subject: EventSubject;
  kind: EventKind;
  date: GenealogicalDate | null;
  place: PlaceId | null;
  notes: string | null;
}

export interface Place {
  id: PlaceId;
  name: string;
  latitude: number | null;
  longitude: number | null;
  parent: PlaceId | null;
}

// — media: a media row + the gallery view (mirrors kith_core::model) —
export interface Media {
  id: MediaId;
  path: string; // relative to the media folder beside the DB
  caption: string | null;
  mime: string | null;
}

export interface MediaItem {
  media: Media;
  is_primary: boolean;
}

// — sources & citations: the evidence layer (mirrors kith_core::model) —
export interface Source {
  id: SourceId;
  title: string;
  author: string | null;
  publication: string | null;
  repository: string | null;
  notes: string | null;
}

export interface Citation {
  id: CitationId;
  source: SourceId;
  subject: CitationSubject;
  page: string | null;
  detail: string | null;
  confidence: Confidence | null;
}

/** A citation with its source resolved alongside (no N+1) — `citations_for`. */
export interface CitationItem {
  citation: Citation;
  source: Source;
}

// — search: a ranked hit from the FTS5-backed `search` command —
/** A ranked search result: the matched person plus an optional "why-matched"
 *  snippet (a maiden name, a birthplace, …) shown as a subtitle. The bm25 score
 *  stays server-side (it only orders the list). */
export interface SearchHit {
  individual: Individual;
  context: string | null;
}

// — composite read views (kith_core::query) —
export interface PersonView {
  individual: Individual;
  names: Name[];
  events: Event[];
  partner_in: FamilyId[];
  child_in: FamilyId[];
}

/** `#[serde(flatten)]` inlines the membership link beside the child record. */
export type ChildView = ChildLink & { individual: Individual };

export interface FamilyView {
  family: Family;
  partner1: Individual | null;
  partner2: Individual | null;
  children: ChildView[];
  events: Event[];
}

export interface EventView {
  event: Event;
  place: Place | null;
  citations: CitationItem[]; // provenance, each source resolved
}

/** A source with the facts it supports — backs the Sources management surface. */
export interface SourceView {
  source: Source;
  citations: Citation[];
}

// — source/citation drafts (create side) —
export interface NewSource {
  title: string;
  author: string | null;
  publication: string | null;
  repository: string | null;
  notes: string | null;
}

export interface NewCitation {
  source: SourceId;
  subject: CitationSubject;
  page: string | null;
  detail: string | null;
  confidence: Confidence | null;
}

// — drafts (create side) —
export interface NewIndividual {
  given_name: string | null;
  surname: string | null;
  name_prefix: string | null;
  name_suffix: string | null;
  nickname: string | null;
  sex: Sex;
  living: boolean;
  notes: string | null;
}

export interface NewFamily {
  partner1: PersonId | null;
  partner2: PersonId | null;
  union_type: UnionType;
  notes: string | null;
}

export interface NewName {
  individual_id: PersonId;
  kind: NameKind;
  given_name: string | null;
  surname: string | null;
  name_prefix: string | null;
  name_suffix: string | null;
  sort_order: number;
}

// — request DTOs (date-bearing payloads carry the raw string; the command
//   parses it via the core date subsystem — no date math in the frontend) —
export interface NewEventRequest {
  subject: EventSubject;
  kind: EventKind;
  date: string | null;
  place_id: PlaceId | null;
  place_name: string | null;
  notes: string | null;
}

export interface UpdateEventRequest {
  id: EventId;
  kind: EventKind;
  date: string | null;
  place_id: PlaceId | null;
  place_name: string | null;
  notes: string | null;
}

// — small command results —
export interface DbInfo {
  path: string;
  schema_version: number;
}

// The result of a successful undo (kith-tauri's UndoOutcome): the
// restored entity family (`"person"`, `"event"`, … — keyed off to refresh the
// right views), a human label for the "Restored …" confirmation, and the number
// of deletions still on the session stack.
export interface UndoOutcome {
  kind: string;
  label: string;
  remaining: number;
}

export interface DatePreview {
  short: string;
  long: string;
  modifier: DateModifier;
}

// The app's identity (kith-tauri's AboutInfo) — surfaced to the About
// modal: product name, the version (the one source of truth), the frozen bundle
// identifier, the MIT license, the repository URL (shown as selectable text), and
// the author line. Plain snake_case wire (single-word fields), like DbInfo.
export interface AboutInfo {
  name: string;
  version: string;
  identifier: string;
  license: string;
  repository: string;
  authors: string;
}

// GEDCOM import result (kith_core::gedcom::ImportSummary). Snake_case keys mirror the
// wire — kith-core structs are NOT rename_all="camelCase" (cf. DbInfo.schema_version).
// Distinct from the api wrappers' camelCase ARG keys (outPath/filePath) — that casing
// is Tauri's JS→Rust param mapping; this is a returned serde struct.
export interface ImportSummary {
  individuals: number;
  families: number;
  events: number;
  names: number;
  places: number;
  skipped_tags: Record<string, number>; // tag → count, sorted/deterministic (e.g. { SOUR: 2 })
}

// The outcome of a fresh-tree GEDCOM import (kith-tauri's GedcomImport): the newly
// created + opened database, plus what was read. The GUI import always makes a NEW
// tree (it never appends into the open DB — additive --merge is CLI-only).
export interface GedcomImport {
  db: DbInfo;
  summary: ImportSummary;
}

// The outcome of a fresh-tree "LB" JSON import (kith-tauri's LbImport) — the same
// shape as GedcomImport (a new DB + the shared ImportSummary). Like the GEDCOM
// import, it always makes a NEW tree. LB carries no alternate names / media /
// sources, so those summary counts stay 0.
export interface LbImport {
  db: DbInfo;
  summary: ImportSummary;
}

// — layout model (kith_core::layout; the positioned chart the canvas renders) —
export type ChartMode = "Ancestors" | "Descendants" | "Hourglass" | "Network"; // Network: whole-graph layered layout; ignores depth

// The exported document's palette, on the IPC wire (serde variant names — capitalised).
// NOTE: distinct from the app's live-canvas UI theme ("light"/"dark", ThemeToggle /
// the `data-theme` attribute) — do NOT lowercase these or the `export_html` Theme fails.
export type Theme = "Light" | "Dark";
export type NodeKind = "Person" | "Union";
export type LinkKind = "Descent" | "Partner";
export type NodeId = number; // #[serde(transparent)] u32 → bare number
export type NodeEntity = { Person: PersonId } | { Union: FamilyId }; // externally tagged

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface Point {
  x: number;
  y: number;
}

export interface NodeContent {
  display_name: string;
  lifespan: string | null; // already-formatted ("1887–1956", "b. 1990"); never parsed in TS
  sex: Sex;
  living: boolean;
  portrait: MediaId | null; // the person's primary portrait, resolved to a URL by the media store
}

export interface LayoutNode {
  id: NodeId;
  kind: NodeKind;
  entity: NodeEntity;
  x: number;
  y: number;
  width: number;
  height: number;
  content: NodeContent | null; // Some for persons, null for unions — always present
  focal: boolean;
}

export interface LayoutLink {
  from: NodeId;
  to: NodeId;
  kind: LinkKind;
  anchors: Point[];
}

export interface LayoutModel {
  mode: ChartMode;
  nodes: LayoutNode[];
  links: LayoutLink[];
  bounds: Rect;
}
