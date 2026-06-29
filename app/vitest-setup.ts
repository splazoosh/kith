// vitest-setup.ts — the jsdom component-test harness.
//
// `@testing-library/jest-dom/vitest` extends vitest's `expect` with the DOM
// matchers (`toBeInTheDocument`, …); `afterEach(cleanup)` unmounts every
// rendered component (and runs its onDestroy — clearing the DateInput debounce)
// so tests stay isolated.

import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
});
