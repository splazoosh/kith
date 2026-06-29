// drafts.test.ts — node-env vitest for the pure draft helpers. No DOM, no
// `invoke`: factories return the core's `New*` defaults and the event-kind
// helpers round-trip the open `Other` arm and the known variants.

import { describe, expect, test } from "vitest";

import {
  emptyFamily,
  emptyIndividual,
  eventKindLabel,
  eventKindSelect,
  KNOWN_EVENT_KINDS,
  parseEventKind,
  toIndividualDraft,
} from "./drafts";
import type { Individual } from "./types";

describe("emptyIndividual", () => {
  test("mirrors the core NewIndividual defaults (living, Unknown sex, nulls)", () => {
    expect(emptyIndividual()).toEqual({
      given_name: null,
      surname: null,
      name_prefix: null,
      name_suffix: null,
      nickname: null,
      sex: "Unknown",
      living: true,
      notes: null,
    });
  });
});

describe("emptyFamily", () => {
  test("no partners, Unknown union, no notes", () => {
    expect(emptyFamily()).toEqual({
      partner1: null,
      partner2: null,
      union_type: "Unknown",
      notes: null,
    });
  });
});

describe("toIndividualDraft", () => {
  test("copies every NewIndividual field and drops the id", () => {
    const rec: Individual = {
      id: 42,
      given_name: "Jane",
      surname: "Doe",
      name_prefix: null,
      name_suffix: null,
      nickname: "Janie",
      sex: "Female",
      living: false,
      notes: "a note",
    };
    const draft = toIndividualDraft(rec);
    expect(draft).toEqual({
      given_name: "Jane",
      surname: "Doe",
      name_prefix: null,
      name_suffix: null,
      nickname: "Janie",
      sex: "Female",
      living: false,
      notes: "a note",
    });
    expect("id" in draft).toBe(false);
  });
});

describe("event-kind mapping", () => {
  test("the Other arm round-trips its inner code", () => {
    expect(eventKindLabel(parseEventKind("Other", "christening"))).toBe(
      "christening",
    );
    expect(parseEventKind("Other", "  baptism in absentia  ")).toEqual({
      Other: "baptism in absentia",
    });
  });

  test("a known kind round-trips by variant name", () => {
    for (const k of KNOWN_EVENT_KINDS) {
      const kind = parseEventKind(k, "ignored");
      expect(kind).toBe(k);
      expect(eventKindLabel(kind)).toBe(k);
      expect(eventKindSelect(kind)).toBe(k);
    }
  });

  test("eventKindSelect collapses an Other kind to the sentinel", () => {
    expect(eventKindSelect({ Other: "christening" })).toBe("Other");
  });
});
