/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";

// Tauri's devUrl pins a fixed port; strictPort fails fast on a clash rather than
// silently moving off it (which would leave the Tauri window pointing nowhere).
export default defineConfig({
  // `svelteTesting()` adds the `browser` resolve condition under test so Svelte 5
  // compiles to its client runtime for component tests.
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/crates/kith-tauri/**"] },
  },
  build: {
    outDir: "dist",
    target: "esnext",
    sourcemap: true,
  },
  test: {
    // jsdom for the component suite; the node-style api/format/drafts suites run
    // unchanged under it. The setup file registers jest-dom + afterEach cleanup.
    environment: "jsdom",
    setupFiles: ["./vitest-setup.ts"],
    include: ["src/**/*.{test,spec}.ts"],
  },
});
