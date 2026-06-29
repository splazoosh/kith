// media.svelte.test.ts — the portrait-resolution store: it batches a
// model's distinct portrait ids into ONE media_paths IPC and maps them to asset
// URLs; a portrait-less (or null) model makes no IPC and clears the cache.

import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../api", () => ({
  mediaPaths: vi.fn(),
  assetUrl: vi.fn((p: string) => `asset://${p}`),
}));

import * as api from "../api";
import type { LayoutModel } from "../types";
import { media } from "./media.svelte";

const mediaPaths = vi.mocked(api.mediaPaths);

/** A model whose person nodes carry the given portrait ids (null = no portrait). */
function modelWith(portraits: (number | null)[]): LayoutModel {
  return {
    mode: "Descendants",
    nodes: portraits.map((portrait, i) => ({
      id: i,
      kind: "Person",
      entity: { Person: i + 1 },
      x: 0,
      y: 0,
      width: 220,
      height: 72,
      content: {
        display_name: `P${i}`,
        lifespan: null,
        sex: "Unknown",
        living: false,
        portrait,
      },
      focal: i === 0,
    })),
    links: [],
    bounds: { x: 0, y: 0, width: 220, height: 72 },
  };
}

beforeEach(() => {
  mediaPaths.mockReset();
  media.clear();
});

test("resolvePortraits batches the distinct ids into ONE call and maps to asset URLs", async () => {
  mediaPaths.mockResolvedValue({ 1: "/m/1.png", 5: "/m/5.jpg" });
  await media.resolvePortraits(modelWith([1, null, 5, 1])); // 1 appears twice → deduped

  expect(mediaPaths).toHaveBeenCalledTimes(1);
  expect(mediaPaths).toHaveBeenCalledWith([1, 5]);
  expect(media.url(1)).toBe("asset:///m/1.png");
  expect(media.url(5)).toBe("asset:///m/5.jpg");
  expect(media.url(9)).toBeNull(); // unresolved id
  expect(media.url(null)).toBeNull();
});

test("a portrait-less model makes no IPC and clears any prior cache", async () => {
  mediaPaths.mockResolvedValue({ 1: "/m/1.png" });
  await media.resolvePortraits(modelWith([1]));
  expect(media.url(1)).not.toBeNull();

  mediaPaths.mockClear();
  await media.resolvePortraits(modelWith([null, null]));
  expect(mediaPaths).not.toHaveBeenCalled();
  expect(media.url(1)).toBeNull();
});

test("a null model makes no IPC", async () => {
  await media.resolvePortraits(null);
  expect(mediaPaths).not.toHaveBeenCalled();
});
