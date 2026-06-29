// errors.ts — the one place a rejected `invoke` becomes a typed error.
//
// The GUI counterpart to the CLI consuming the exit-code contract: a Tauri
// command's `Err(CommandError)` arrives as the serialized `{ kind, message }`
// object; `fromInvokeError` normalizes it (and any non-contract rejection) into
// a `CommandError` class so every component catches one uniform shape and never
// `try/catch`es a raw value.

export type ErrorKind =
  | "not_found"
  | "validation"
  | "io"
  | "database"
  | "unexpected";

/** A typed IPC failure: the `kind` the UI narrows on plus an actionable message. */
export class CommandError extends Error {
  readonly kind: ErrorKind;

  constructor(kind: ErrorKind, message: string) {
    super(message);
    this.name = "CommandError";
    this.kind = kind;
  }
}

/** Type guard: is `e` a `CommandError`? */
export function isCommandError(e: unknown): e is CommandError {
  return e instanceof CommandError;
}

const KINDS = new Set<ErrorKind>([
  "not_found",
  "validation",
  "io",
  "database",
  "unexpected",
]);

function isErrorKind(k: unknown): k is ErrorKind {
  return typeof k === "string" && KINDS.has(k as ErrorKind);
}

/**
 * Normalize whatever a rejected `invoke` threw into a typed `CommandError`.
 *
 * Tauri rejects a command's `Err(CommandError)` as the serialized object
 * `{ kind, message }`; a non-contract rejection (a plugin fault, a serialization
 * error, a thrown string) collapses to `kind: "unexpected"`.
 */
export function fromInvokeError(e: unknown): CommandError {
  if (e && typeof e === "object" && "kind" in e && "message" in e) {
    const { kind, message } = e as { kind: unknown; message: unknown };
    if (isErrorKind(kind) && typeof message === "string") {
      return new CommandError(kind, message);
    }
  }
  return new CommandError(
    "unexpected",
    typeof e === "string" ? e : "an unexpected error occurred",
  );
}

/**
 * Coerce an already-thrown value (from a `catch`) to a `CommandError`. Values
 * thrown by {@link CommandError} pass through; anything else is wrapped as
 * `unexpected`. Used at the store `catch` sites that feed the toast channel.
 */
export function asCommandError(e: unknown): CommandError {
  if (isCommandError(e)) return e;
  return new CommandError(
    "unexpected",
    e instanceof Error ? e.message : "an unexpected error occurred",
  );
}
