// DateInput.test.ts — jsdom component test. `parse_date` is mocked
// the way api.test.ts mocks `invoke`. Proves the three contract pieces: the
// success preview, the local (non-toasting) unrecognized hint, and raw
// passthrough — the value the field exposes is always exactly what was typed.

import { fireEvent, render } from "@testing-library/svelte";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

vi.mock("../lib/api", () => ({ parseDate: vi.fn() }));

import DateInput from "./DateInput.svelte";
import * as api from "../lib/api";
import { CommandError } from "../lib/errors";
import { toast } from "../lib/stores/toast.svelte";

const parseDate = vi.mocked(api.parseDate);

beforeEach(() => {
  parseDate.mockReset();
  toast.items = [];
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

test("a recognized date renders the long form and the modifier chip", async () => {
  parseDate.mockResolvedValue({ short: "c. 1850", long: "about 1850", modifier: "About" });
  const { getByLabelText, getByText } = render(DateInput, {
    props: { value: "", label: "Birth", id: "birth" },
  });

  await fireEvent.input(getByLabelText("Birth"), { target: { value: "ABT 1850" } });
  await vi.advanceTimersByTimeAsync(250);

  expect(parseDate).toHaveBeenCalledWith("ABT 1850");
  expect(getByText("about 1850")).toBeInTheDocument();
  expect(getByText("About")).toBeInTheDocument();
});

test("an unparseable string shows the local hint and pushes NO toast", async () => {
  parseDate.mockRejectedValue(new CommandError("validation", "unrecognized date"));
  const pushError = vi.spyOn(toast, "pushError");
  const { getByLabelText, getByText } = render(DateInput, {
    props: { value: "", label: "Birth", id: "birth" },
  });

  await fireEvent.input(getByLabelText("Birth"), { target: { value: "qwerty" } });
  await vi.advanceTimersByTimeAsync(250);

  expect(getByText(/unrecognized — will be saved as written/)).toBeInTheDocument();
  expect(pushError).not.toHaveBeenCalled();
  expect(toast.items).toHaveLength(0);
});

test("the input value is always the raw typed string, regardless of preview", async () => {
  // No timer advance: the debounce never fires, so no preview ever resolves —
  // yet the field still reflects exactly what was typed (raw passthrough).
  const { getByLabelText } = render(DateInput, {
    props: { value: "", label: "Death", id: "death" },
  });

  const input = getByLabelText("Death") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "BET 1850 AND 1860" } });

  expect(input.value).toBe("BET 1850 AND 1860");
  expect(parseDate).not.toHaveBeenCalled(); // debounce pending, not yet fired
});
