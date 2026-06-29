// mediaActions.ts — the media picker→import→toast orchestration,
// mirroring exportActions.ts. The dialog plugin returns a path STRING; the Rust
// `media_import` command copies the file into the media folder (least privilege
// — no `fs` ACL added). The gallery component just calls this and reloads.

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import * as api from "./api";
import { asCommandError } from "./errors";
import { toast } from "./stores/toast.svelte";
import type { MediaItem, MediaSubject } from "./types";

const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "gif", "webp"];

/** Pick an image via the native open dialog and import it for `subject`,
 *  copying it into the media folder. Returns the created item, or null if the
 *  dialog was cancelled (the `pickAndCreate`/`exportChart` precedent). */
export async function pickAndImportMedia(
  subject: MediaSubject,
  isPrimary: boolean,
): Promise<MediaItem | null> {
  const path = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
  });
  if (typeof path !== "string") return null; // cancelled
  try {
    const item = await api.mediaImport(subject, path, isPrimary);
    toast.pushNotice("Photo added.");
    return item;
  } catch (e) {
    toast.pushError(asCommandError(e));
    return null;
  }
}
