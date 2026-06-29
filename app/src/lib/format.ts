// format.ts — the frontend's only "logic": pure display helpers, kept node-
// testable and framework-free. They assemble strings for display; they do NOT
// parse or reconstruct dates. Per the date guardrail a lifespan reads only
// the integer `year` off an already-parsed `GenealogicalDate` — string parsing
// and modifier handling live behind the `parse_date` command, never here.

import type { Event, Family, GenealogicalDate, Individual } from "./types";

/**
 * The name parts `displayName` reads — satisfied by `Individual`, `NewIndividual`,
 * and `Name` alike (the latter has no `nickname`, hence its optionality).
 */
export interface NameParts {
  name_prefix: string | null;
  given_name: string | null;
  surname: string | null;
  name_suffix: string | null;
  nickname?: string | null;
}

/**
 * A person's display name: `prefix given surname suffix`, falling back to the
 * nickname, then `"(unnamed)"`. Blank/whitespace parts are dropped.
 */
export function displayName(p: NameParts): string {
  const parts = [p.name_prefix, p.given_name, p.surname, p.name_suffix]
    .map((s) => s?.trim() ?? "")
    .filter((s) => s.length > 0);
  if (parts.length > 0) return parts.join(" ");
  const nick = p.nickname?.trim();
  if (nick) return nick;
  return "(unnamed)";
}

/** The integer year of an already-parsed date (the `from` bound of a range). */
function yearOf(date: GenealogicalDate | null): number | null {
  if (!date) return null;
  if ("Single" in date) return date.Single.date.year;
  return date.Range.from.year;
}

/**
 * The integer year of an already-parsed date as a string (`""` if undated) —
 * the events-list projection. Integer-year only, no modifier/formatting:
 * the full human form comes from the `parse_date` command, never from here.
 */
export function dateYear(date: GenealogicalDate | null): string {
  const y = yearOf(date);
  return y === null ? "" : String(y);
}

/**
 * A `"1850–1910"` lifespan from a person's events: the year of the first birth
 * and first death event. Open-ended forms (`"1850–"`, `"–1910"`) and `""` (no
 * dated birth/death) are returned as-is. Integer-year projection only.
 */
export function lifespanYears(events: Event[]): string {
  const birth = events.find((e) => e.kind === "Birth");
  const death = events.find((e) => e.kind === "Death");
  const b = birth ? yearOf(birth.date) : null;
  const d = death ? yearOf(death.date) : null;
  if (b === null && d === null) return "";
  return `${b ?? ""}–${d ?? ""}`;
}

/**
 * A family's display label (`"Doe × Roe"`) from its resolved partners'
 * surnames (falling back to each partner's display name). `"(unlinked family)"`
 * when no partner resolves. A display join, not domain logic.
 */
export function familyLabel(
  f: Family,
  people: Map<number, Individual>,
): string {
  const names = [f.partner1, f.partner2]
    .map((id) => (id === null ? undefined : people.get(id)))
    .filter((p): p is Individual => p !== undefined)
    .map((p) => p.surname?.trim() || displayName(p));
  if (names.length === 0) return "(unlinked family)";
  return names.join(" × ");
}

/**
 * Client-side family filter (no `family_search` command exists): keeps
 * families whose label contains `query` (case-insensitive). An empty query
 * passes everything through.
 */
export function filterFamilies(
  families: Family[],
  people: Map<number, Individual>,
  query: string,
): Family[] {
  const q = query.trim().toLowerCase();
  if (q === "") return families;
  return families.filter((f) =>
    familyLabel(f, people).toLowerCase().includes(q),
  );
}
