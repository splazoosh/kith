<script lang="ts">
  // MediaGallery — list / add / set-portrait / remove a subject's photos. Self-
  // contained like NamesEditor: it loads `media_for` for its `subject` and
  // reloads after each write. Thumbnails stream over the asset protocol
  // (`media_paths` → `convertFileSrc`); the picker + import + toast live in
  // mediaActions (the dialog convention). The first photo added becomes the
  // primary (portrait); "Set as portrait" re-primaries; remove confirms via a toast.

  import * as api from "../lib/api";
  import { asCommandError } from "../lib/errors";
  import { pickAndImportMedia } from "../lib/mediaActions";
  import { toast } from "../lib/stores/toast.svelte";
  import { undo } from "../lib/stores/undo.svelte";
  import type { MediaId, MediaItem, MediaSubject } from "../lib/types";

  interface Props {
    subject: MediaSubject;
  }
  let { subject }: Props = $props();

  let items = $state<MediaItem[]>([]);
  let urls = $state<Record<number, string>>({});
  let busy = $state(false);

  // (Re)load whenever the subject changes — keyed on its JSON, since it is an object.
  $effect(() => {
    void JSON.stringify(subject);
    void load();
  });

  async function load(): Promise<void> {
    try {
      items = await api.mediaFor(subject);
      await resolveUrls();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  async function resolveUrls(): Promise<void> {
    const ids = items.map((i) => i.media.id);
    if (ids.length === 0) {
      urls = {};
      return;
    }
    const paths = await api.mediaPaths(ids);
    const next: Record<number, string> = {};
    for (const [id, p] of Object.entries(paths)) next[Number(id)] = api.assetUrl(p);
    urls = next;
  }

  async function add(): Promise<void> {
    busy = true;
    // The first photo is the subject's primary (portrait); later ones are not.
    const item = await pickAndImportMedia(subject, items.length === 0);
    if (item) await load();
    busy = false;
  }

  async function setPrimary(id: MediaId): Promise<void> {
    try {
      await api.mediaSetPrimary(id, subject);
      await load();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }

  async function remove(id: MediaId, label: string): Promise<void> {
    try {
      await api.mediaDelete(id);
      undo.recordDelete(label);
      await load();
    } catch (e) {
      toast.pushError(asCommandError(e));
    }
  }
</script>

<section class="media">
  <div class="head">
    <h3>Photos</h3>
    <button type="button" class="link" onclick={add} disabled={busy}>
      + Add photo
    </button>
  </div>

  {#if items.length > 0}
    <ul role="list" class="grid">
      {#each items as item (item.media.id)}
        <li class:primary={item.is_primary}>
          {#if urls[item.media.id]}
            <img src={urls[item.media.id]} alt={item.media.caption ?? "Photo"} />
          {:else}
            <div class="placeholder" aria-hidden="true"></div>
          {/if}
          <div class="row">
            {#if item.is_primary}
              <span class="badge">Portrait</span>
            {:else}
              <button
                type="button"
                class="link"
                onclick={() => setPrimary(item.media.id)}
              >
                Set as portrait
              </button>
            {/if}
            <button
              type="button"
              class="link danger"
              aria-label="Remove photo"
              onclick={() => remove(item.media.id, item.media.caption ?? "the photo")}
            >
              Remove
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="hint">No photos.</p>
  {/if}
</section>

<style>
  .media {
    margin-top: var(--space-6);
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  h3 {
    font-size: var(--text-md);
    color: var(--color-ink-soft);
  }

  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(7rem, 1fr));
    gap: var(--space-3);
  }

  li {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  img,
  .placeholder {
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-hairline);
    background: var(--color-surface-2);
  }

  li.primary img,
  li.primary .placeholder {
    border-color: var(--color-accent);
    border-width: 2px;
  }

  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-2);
  }

  .badge {
    font-size: var(--text-xs);
    color: var(--color-accent-text);
  }

  .hint {
    color: var(--color-ink-soft);
    font-size: var(--text-sm);
  }

  .link {
    background: transparent;
    border: none;
    color: var(--color-accent-text);
    font-size: var(--text-sm);
    padding: 0;
  }

  .link.danger {
    color: var(--color-danger);
  }
</style>
