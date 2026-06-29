// modal.svelte.ts — a global "is any modal open?" flag.
//
// The keyboard-shortcut registry stands down while a dialog or palette owns the
// keyboard (the modals keep their own Esc/Tab traps). Each modal registers while
// it is mounted/open and de-registers on close; a depth counter handles the
// (rare) nested case. Wire it with one line in the modal's script:
//
//   • conditionally-mounted modals (ExportDialog, ConfirmDialog, ShortcutsHelp):
//       $effect(() => modal.open());            // open on mount, close on unmount
//   • always-mounted, self-gated modals (JumpToPerson):
//       $effect(() => { if (store.isOpen) return modal.open(); });
//
// The depth is a PLAIN field, not `$state`: `isOpen` is read imperatively from a
// keydown handler (never in a reactive context), and making it reactive would let
// the registering `$effect` track the read/write `open()` does and loop forever.

class ModalStore {
  #depth = 0;

  /** Whether at least one modal is currently open. */
  get isOpen(): boolean {
    return this.#depth > 0;
  }

  /** Register an open modal; returns a one-shot closer for the unmount cleanup. */
  open(): () => void {
    this.#depth += 1;
    let closed = false;
    return () => {
      if (closed) return;
      closed = true;
      this.#depth -= 1;
    };
  }
}

export const modal = new ModalStore();
