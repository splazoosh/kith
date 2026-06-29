// about.svelte.ts — the About/Help modal open state.
//
// A tiny runes store mirroring `shortcutsHelp` / `searchPalette`: open()/close()
// + a read-only `isOpen`. App mounts `AboutModal` while `isOpen`; the modal
// fetches `aboutInfo()` on mount and renders the app's identity. No logic here —
// just the open flag (the DatabaseBar "About" button toggles it).

class AboutStore {
  #open = $state(false);

  /** Whether the About modal is open. */
  get isOpen(): boolean {
    return this.#open;
  }

  /** Open the About modal. */
  open(): void {
    this.#open = true;
  }

  /** Close the About modal. */
  close(): void {
    this.#open = false;
  }
}

export const about = new AboutStore();
