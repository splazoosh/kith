// drafts.ts — pure, framework-free draft factories and the event-kind mapping.
//
// The only "logic" the write forms need, kept OUT of the components so it is
// node-testable and free of Svelte/`invoke`. Factories mirror the core's
// `New*` defaults verbatim (an unnamed person is valid, `living: true`); the
// event-kind helpers translate between the open `EventKind` wire shape and the
// flat <select> the editor renders. No date math lives here — the date
// field round-trips raw strings through the `parse_date` command.

import type {
  EventKind,
  Individual,
  NewFamily,
  NewIndividual,
} from "./types";

/** A blank person draft: all fields empty, `sex: "Unknown"`, `living: true`. */
export function emptyIndividual(): NewIndividual {
  return {
    given_name: null,
    surname: null,
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Unknown",
    living: true,
    notes: null,
  };
}

/** The edit form's working copy of a person: the record minus its immutable id. */
export function toIndividualDraft(i: Individual): NewIndividual {
  return {
    given_name: i.given_name,
    surname: i.surname,
    name_prefix: i.name_prefix,
    name_suffix: i.name_suffix,
    nickname: i.nickname,
    sex: i.sex,
    living: i.living,
    notes: i.notes,
  };
}

/** A blank family draft: no partners, `union_type: "Unknown"`, no notes. */
export function emptyFamily(): NewFamily {
  return {
    partner1: null,
    partner2: null,
    union_type: "Unknown",
    notes: null,
  };
}

/** The eight known `EventKind` variant names, for the editor's <select>. */
export const KNOWN_EVENT_KINDS = [
  "Birth",
  "Death",
  "Marriage",
  "Divorce",
  "Baptism",
  "Burial",
  "Residence",
  "Occupation",
] as const;

/** The select sentinel revealing the free-text input for an open `Other` kind. */
export const OTHER_KIND = "Other" as const;

/** A human label for an event kind: the variant name, or the inner `Other` code. */
export function eventKindLabel(kind: EventKind): string {
  return typeof kind === "string" ? kind : kind.Other;
}

/**
 * Map a `<select>` value (a known variant name, or `"Other"`) plus the revealed
 * free-text back to an `EventKind`. `"Other"` yields `{ Other: <trimmed text> }`;
 * any other value is a known variant name passed through verbatim.
 */
export function parseEventKind(select: string, other: string): EventKind {
  if (select === OTHER_KIND) return { Other: other.trim() };
  return select as EventKind;
}

/** The `<select>` value for an event kind: the variant name, or `"Other"`. */
export function eventKindSelect(kind: EventKind): string {
  return typeof kind === "string" ? kind : OTHER_KIND;
}
