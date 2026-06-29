// ExportDialog.test.ts — jsdom component test for the export flow.
// Renders the real ExportDialog and drives it through the export action with the
// native save dialog + api mocked, asserting the outcomes: write-on-path (+ a
// notice), the include-living opt-out on the wire, a silent cancel, and an error
// toast. The renderer's output is the Rust snapshots' job — this asserts the
// FLOW (which fn, which args), not the HTML.

import { fireEvent, render } from "@testing-library/svelte";
import { beforeEach, expect, test, vi } from "vitest";

const { save } = vi.hoisted(() => ({ save: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));
vi.mock("../lib/api", () => ({ exportHtml: vi.fn() }));

import * as api from "../lib/api";
import { CommandError } from "../lib/errors";
import { chart } from "../lib/stores/chart.svelte";
import { toast } from "../lib/stores/toast.svelte";
import ExportDialog from "./ExportDialog.svelte";

const exportHtml = vi.mocked(api.exportHtml);
const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  save.mockReset();
  exportHtml.mockReset();
  toast.items = [];
  chart.clear(); // mode → "Descendants", generations → 4 (the dialog seeds from these)
});

function open() {
  return render(ExportDialog, {
    props: { rootId: 11, focalName: "Ada Lovelace", onclose: vi.fn() },
  });
}

test("a chosen path exports the chart with the seeded options and pushes a notice", async () => {
  save.mockResolvedValue("C:/out/tree.html");
  exportHtml.mockResolvedValue(undefined);

  const { getByRole } = open();
  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  // The save dialog seeds its default name from focal + mode.
  expect(save).toHaveBeenCalledWith({
    defaultPath: "Ada Lovelace Descendants.html",
    filters: [{ name: "HTML", extensions: ["html"] }],
  });
  // The IPC call carries the seeded options + the chosen path; redaction on by default.
  expect(exportHtml).toHaveBeenCalledWith(
    11,
    "Descendants",
    4,
    "Light",
    false,
    false, // portraits off by default
    "C:/out/tree.html",
  );
  expect(toast.items.at(-1)).toMatchObject({ kind: "notice" });
});

test("ticking Include living opts out of redaction on the wire", async () => {
  save.mockResolvedValue("C:/out/tree.html");
  exportHtml.mockResolvedValue(undefined);

  const { getByRole, getByLabelText } = open();
  await fireEvent.click(getByLabelText("Include living individuals' details"));
  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  expect(exportHtml).toHaveBeenCalledWith(
    11,
    "Descendants",
    4,
    "Light",
    true, // includeLiving — the only opt-out, passed INTO the command (never redacted in TS)
    false, // portraits off by default
    "C:/out/tree.html",
  );
});

test("ticking Include portraits passes the flag on the wire", async () => {
  save.mockResolvedValue("C:/out/tree.html");
  exportHtml.mockResolvedValue(undefined);

  const { getByRole, getByLabelText } = open();
  await fireEvent.click(getByLabelText("Include portraits"));
  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  expect(exportHtml).toHaveBeenCalledWith(
    11,
    "Descendants",
    4,
    "Light",
    false,
    true, // portraits — embed each person's primary portrait (base64)
    "C:/out/tree.html",
  );
});

test("offers Network as an export mode and exports it (depth ignored)", async () => {
  save.mockResolvedValue("C:/out/net.html");
  exportHtml.mockResolvedValue(undefined);

  const { getByRole, getByLabelText } = open();
  // Network is a selectable mode option.
  expect(getByRole("option", { name: "Network" })).toBeTruthy();

  // Select it; the depth slider becomes inert, and the export carries "Network".
  await fireEvent.change(getByLabelText("Mode"), {
    target: { value: "Network" },
  });
  const depth = getByLabelText("Generation depth") as HTMLInputElement;
  expect(depth.disabled).toBe(true);

  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  expect(exportHtml).toHaveBeenCalledWith(
    11,
    "Network",
    expect.any(Number),
    "Light",
    false,
    false, // portraits off by default
    "C:/out/net.html",
  );
});

test("cancelling the save dialog is a silent no-op", async () => {
  save.mockResolvedValue(null);

  const { getByRole } = open();
  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  expect(exportHtml).not.toHaveBeenCalled();
  expect(toast.items).toHaveLength(0);
});

test("a failed export surfaces a sticky error toast", async () => {
  save.mockResolvedValue("C:/out/tree.html");
  exportHtml.mockRejectedValue(new CommandError("io", "permission denied"));

  const { getByRole } = open();
  await fireEvent.click(getByRole("button", { name: "Export…" }));
  await flush();

  expect(toast.items.at(-1)).toMatchObject({
    kind: "io",
    message: "permission denied",
    sticky: true,
  });
});
