// toast.svelte.ts — the single error/notice channel every store `catch` funnels
// into. Errors are sticky (the user dismisses them); notices auto-dismiss.

import type { CommandError, ErrorKind } from "../errors";

/** A single action button on a non-error toast ("Deleted X · Undo"). */
export interface ToastAction {
  label: string;
  run: () => void;
}

export interface Toast {
  id: number;
  kind: ErrorKind | "notice";
  message: string;
  sticky: boolean;
  action?: ToastAction;
}

const NOTICE_TTL_MS = 4000;
// An action toast lingers longer than a plain notice — the user needs time to
// reach for "Undo".
const ACTION_TTL_MS = 8000;

class ToastStore {
  items = $state<Toast[]>([]);
  #nextId = 1;
  /** The live action toast, so a new one replaces the previous (one at a time). */
  #actionId: number | null = null;

  #push(
    kind: Toast["kind"],
    message: string,
    sticky: boolean,
    action?: ToastAction,
  ): number {
    const id = this.#nextId++;
    this.items = [...this.items, { id, kind, message, sticky, action }];
    return id;
  }

  /** Surface a failed command as a sticky toast tagged with its `kind`. */
  pushError(e: CommandError): void {
    this.#push(e.kind, e.message, true);
  }

  /** Show a transient, self-dismissing notice. */
  pushNotice(message: string): void {
    const id = this.#push("notice", message, false);
    setTimeout(() => this.dismiss(id), NOTICE_TTL_MS);
  }

  /**
   * Show a transient notice carrying one action button (e.g. "Deleted Jane · Undo").
   * Only one action toast lives at a time — a new one dismisses the prior. The
   * button runs `onAction` and dismisses the toast.
   */
  pushAction(message: string, actionLabel: string, onAction: () => void): void {
    if (this.#actionId !== null) this.dismiss(this.#actionId);
    const id = this.#push("notice", message, false, {
      label: actionLabel,
      run: () => {
        this.dismiss(id);
        onAction();
      },
    });
    this.#actionId = id;
    setTimeout(() => {
      if (this.#actionId === id) this.#actionId = null;
      this.dismiss(id);
    }, ACTION_TTL_MS);
  }

  dismiss(id: number): void {
    if (this.#actionId === id) this.#actionId = null;
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toast = new ToastStore();
