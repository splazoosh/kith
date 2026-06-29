// toast.svelte.test.ts — the action-toast variant. The "Deleted X
// · Undo" toast is a non-error notice carrying one action button; only one lives
// at a time, its button runs the action and dismisses it, and it auto-dismisses
// on a longer TTL.

import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { toast } from "./toast.svelte";

beforeEach(() => {
  toast.items = [];
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

test("pushAction shows a notice carrying a working action button", () => {
  const onAction = vi.fn();
  toast.pushAction("Deleted Jane", "Undo", onAction);
  const t = toast.items.at(-1);
  expect(t?.kind).toBe("notice");
  expect(t?.message).toBe("Deleted Jane");
  expect(t?.action?.label).toBe("Undo");

  t?.action?.run();
  expect(onAction).toHaveBeenCalledOnce();
  expect(toast.items).toEqual([]); // running the action dismisses the toast
});

test("a new action toast replaces the previous one (one at a time)", () => {
  toast.pushAction("Deleted A", "Undo", vi.fn());
  toast.pushAction("Deleted B", "Undo", vi.fn());
  const actions = toast.items.filter((t) => t.action);
  expect(actions.length).toBe(1);
  expect(actions[0]?.message).toBe("Deleted B");
});

test("an action toast auto-dismisses after its TTL", () => {
  toast.pushAction("Deleted C", "Undo", vi.fn());
  expect(toast.items.length).toBe(1);
  vi.advanceTimersByTime(8000);
  expect(toast.items).toEqual([]);
});
