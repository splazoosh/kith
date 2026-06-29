// format.test.ts — node-env vitest for the pure display helpers.

import { describe, expect, test } from "vitest";

import { displayName, familyLabel, filterFamilies, lifespanYears } from "./format";
import type { Event, Family, Individual, NewIndividual } from "./types";

function person(over: Partial<Individual> = {}): Individual {
  return {
    id: 1,
    given_name: null,
    surname: null,
    name_prefix: null,
    name_suffix: null,
    nickname: null,
    sex: "Unknown",
    living: true,
    notes: null,
    ...over,
  };
}

describe("displayName", () => {
  test("joins prefix, given, surname, suffix when present", () => {
    const p: NewIndividual = {
      given_name: "Jane",
      surname: "Doe",
      name_prefix: "Dr",
      name_suffix: "Jr",
      nickname: null,
      sex: "Female",
      living: false,
      notes: null,
    };
    expect(displayName(p)).toBe("Dr Jane Doe Jr");
  });

  test("drops absent parts", () => {
    expect(displayName(person({ given_name: "John", surname: "Roe" }))).toBe(
      "John Roe",
    );
  });

  test("falls back to nickname, then (unnamed)", () => {
    expect(displayName(person({ nickname: "Red" }))).toBe("Red");
    expect(displayName(person())).toBe("(unnamed)");
  });
});

describe("lifespanYears", () => {
  const birth = (year: number): Event => ({
    id: 1,
    subject: { Individual: 1 },
    kind: "Birth",
    date: { Single: { modifier: "About", date: { year, month: null, day: null } } },
    place: null,
    notes: null,
  });
  const death = (year: number): Event => ({
    id: 2,
    subject: { Individual: 1 },
    kind: "Death",
    date: { Single: { modifier: "Exact", date: { year, month: null, day: null } } },
    place: null,
    notes: null,
  });

  test("both bounds from Single dates", () => {
    expect(lifespanYears([birth(1850), death(1910)])).toBe("1850–1910");
  });

  test("a Range birth reads its from-year", () => {
    const rangeBirth: Event = {
      id: 3,
      subject: { Individual: 1 },
      kind: "Birth",
      date: {
        Range: {
          from: { year: 1849, month: null, day: null },
          to: { year: 1851, month: null, day: null },
        },
      },
      place: null,
      notes: null,
    };
    expect(lifespanYears([rangeBirth])).toBe("1849–");
  });

  test("open-ended and empty forms", () => {
    expect(lifespanYears([death(1910)])).toBe("–1910");
    expect(lifespanYears([])).toBe("");
  });
});

describe("familyLabel / filterFamilies", () => {
  const jane = person({ id: 1, surname: "Doe", given_name: "Jane" });
  const john = person({ id: 2, surname: "Roe", given_name: "John" });
  const people = new Map<number, Individual>([
    [1, jane],
    [2, john],
  ]);
  const fam = (over: Partial<Family>): Family => ({
    id: 1,
    partner1: null,
    partner2: null,
    union_type: "Marriage",
    notes: null,
    ...over,
  });

  test("two partners join with ×", () => {
    expect(familyLabel(fam({ partner1: 1, partner2: 2 }), people)).toBe(
      "Doe × Roe",
    );
  });

  test("one partner shows the single surname", () => {
    expect(familyLabel(fam({ partner1: 1 }), people)).toBe("Doe");
  });

  test("no resolvable partner is unlinked", () => {
    expect(familyLabel(fam({}), people)).toBe("(unlinked family)");
  });

  test("filterFamilies matches the label, case-insensitively", () => {
    const families = [fam({ id: 1, partner1: 1, partner2: 2 }), fam({ id: 2 })];
    expect(filterFamilies(families, people, "roe").map((f) => f.id)).toEqual([1]);
    expect(filterFamilies(families, people, "").length).toBe(2);
  });
});
