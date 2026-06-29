// MediaGallery.test.ts — jsdom test for the photo gallery flow. The api
// + the mediaActions picker are mocked; this asserts the FLOW (load → thumbnails,
// set-primary, delete, add-as-primary-when-empty), not pixels.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({
  mediaFor: vi.fn(),
  mediaPaths: vi.fn(),
  assetUrl: vi.fn((p: string) => `asset://${p}`),
  mediaSetPrimary: vi.fn(),
  mediaDelete: vi.fn(),
}));
const { pickAndImportMedia } = vi.hoisted(() => ({ pickAndImportMedia: vi.fn() }));
vi.mock("../lib/mediaActions", () => ({ pickAndImportMedia }));

import * as api from "../lib/api";
import { toast } from "../lib/stores/toast.svelte";
import type { MediaItem } from "../lib/types";
import MediaGallery from "./MediaGallery.svelte";

const mediaFor = vi.mocked(api.mediaFor);
const mediaPaths = vi.mocked(api.mediaPaths);
const mediaSetPrimary = vi.mocked(api.mediaSetPrimary);
const mediaDelete = vi.mocked(api.mediaDelete);
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function item(id: number, isPrimary: boolean): MediaItem {
  return {
    media: { id, path: `${id}.png`, caption: null, mime: "image/png" },
    is_primary: isPrimary,
  };
}

beforeEach(() => {
  mediaFor.mockReset();
  mediaPaths.mockReset().mockResolvedValue({});
  mediaSetPrimary.mockReset().mockResolvedValue(undefined);
  mediaDelete.mockReset().mockResolvedValue(undefined);
  pickAndImportMedia.mockReset();
  toast.items = [];
});

test("lists a subject's photos, streaming each over the asset protocol", async () => {
  mediaFor.mockResolvedValue([item(1, true), item(2, false)]);
  mediaPaths.mockResolvedValue({ 1: "/m/1.png", 2: "/m/2.png" });

  const { container } = render(MediaGallery, { props: { subject: { Individual: 1 } } });
  await flush();

  expect(mediaFor).toHaveBeenCalledWith({ Individual: 1 });
  const imgs = container.querySelectorAll("img");
  expect(imgs).toHaveLength(2);
  expect(imgs[0].getAttribute("src")).toBe("asset:///m/1.png");
});

test("Set as portrait re-primaries the chosen photo and reloads", async () => {
  mediaFor.mockResolvedValue([item(1, true), item(2, false)]);

  const { getByRole } = render(MediaGallery, { props: { subject: { Individual: 1 } } });
  await flush();
  await fireEvent.click(getByRole("button", { name: "Set as portrait" }));
  await flush();

  expect(mediaSetPrimary).toHaveBeenCalledWith(2, { Individual: 1 });
  expect(mediaFor).toHaveBeenCalledTimes(2); // initial load + reload after the write
});

test("Remove deletes the photo and reloads", async () => {
  mediaFor.mockResolvedValue([item(1, true)]);

  const { getByRole } = render(MediaGallery, { props: { subject: { Individual: 1 } } });
  await flush();
  await fireEvent.click(getByRole("button", { name: "Remove photo" }));
  await flush();

  expect(mediaDelete).toHaveBeenCalledWith(1);
});

test("Add photo imports as primary when the gallery is empty", async () => {
  mediaFor.mockResolvedValue([]);
  pickAndImportMedia.mockResolvedValue(item(1, true));

  const { getByRole } = render(MediaGallery, { props: { subject: { Individual: 1 } } });
  await flush();
  await fireEvent.click(getByRole("button", { name: "+ Add photo" }));
  await flush();

  // The first photo becomes the subject's primary (portrait).
  expect(pickAndImportMedia).toHaveBeenCalledWith({ Individual: 1 }, true);
});
