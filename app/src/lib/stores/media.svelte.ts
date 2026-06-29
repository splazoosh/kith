// media.svelte.ts — portrait URL resolution for the canvas.
//
// The LayoutModel carries each person's primary-portrait MediaId (a lean ref,
// never bytes). This store batches every id in a model into ONE `media_paths`
// IPC per chart (not one per node) and converts the absolute paths to
// asset-protocol URLs (`convertFileSrc`) that TreeNode reads via `url(id)`.
// It streams the file rather than embedding it; the HTML *export* takes the
// separate base64 path (in Rust). A resolution failure leaves a portrait
// unshown — it never blocks the chart.
import * as api from "../api";
import type { LayoutModel, MediaId } from "../types";

class MediaStore {
  // MediaId → asset URL. Reassigned wholesale so `$state` reactivity fires and
  // every `url(id)` reader (TreeNode) re-renders when a batch resolves.
  portraitUrls = $state<Record<number, string>>({});

  /** Batch-resolve every portrait id in `model` to an asset URL (one IPC). */
  async resolvePortraits(model: LayoutModel | null): Promise<void> {
    const ids = collectPortraitIds(model);
    if (ids.length === 0) {
      if (Object.keys(this.portraitUrls).length > 0) this.portraitUrls = {};
      return;
    }
    try {
      const paths = await api.mediaPaths(ids);
      const next: Record<number, string> = {};
      for (const [id, path] of Object.entries(paths)) {
        next[Number(id)] = api.assetUrl(path);
      }
      this.portraitUrls = next;
    } catch {
      // Best-effort: a failed resolve simply leaves portraits unshown.
    }
  }

  /** The resolved asset URL for a portrait id, or null when none/unresolved. */
  url(id: MediaId | null): string | null {
    return id === null ? null : (this.portraitUrls[id] ?? null);
  }

  clear(): void {
    this.portraitUrls = {};
  }
}

export const media = new MediaStore();

/** The distinct, non-null portrait ids carried by a model's person nodes. */
function collectPortraitIds(model: LayoutModel | null): MediaId[] {
  if (!model) return [];
  const ids = new Set<MediaId>();
  for (const node of model.nodes) {
    const portrait = node.content?.portrait;
    if (portrait != null) ids.add(portrait);
  }
  return [...ids];
}
