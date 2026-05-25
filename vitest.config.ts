/// <reference types="vitest" />
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Vitest configuration. Mirrors the production Vite config in shape
// (Svelte plugin enabled so .svelte component tests can mount the
// real components) but with a jsdom environment so DOM APIs work
// without a browser, and Tauri's `@tauri-apps/api/core` aliased to
// our test stub so `invoke()` is mockable per-test.
//
// Tests live next to their subject as `*.test.ts` / `*.test.svelte.ts`
// — keeps the file close to the code it pins and lets you read both
// in a single window.
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    // Replace the real Tauri IPC entry with a tiny mock so every
    // store test can `vi.mock` invoke without touching the actual
    // bridge. The mock file just re-exports a vi.fn(); each test
    // overrides return values via `vi.mocked(invoke).mockResolvedValue(...)`.
    conditions: ["browser"],
  },
  test: {
    environment: "jsdom",
    globals: false, // explicit imports keep the surface smaller
    include: ["src/**/*.{test,spec}.{ts,svelte.ts}"],
    setupFiles: ["./src/test-setup.ts"],
  },
});
