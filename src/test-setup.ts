// Vitest setup — runs once before any test file. Centralises the
// stub of `@tauri-apps/api/core` so individual test files only need
// to override `invoke`'s return value, not re-wire the whole module.
//
// Why mock `core` specifically: every typed wrapper in
// `src/lib/tauri.ts` calls `invoke(commandName, args)`. Tests that
// exercise the wrappers (or the stores that use them) need
// deterministic `invoke` behaviour without spinning up the Tauri
// runtime. The mock here gives them a `vi.fn()` they can configure.

import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/svelte";

// Unmount any components mounted via `render()` between tests.
// @testing-library/svelte's `render()` appends each instance to
// `document.body`; without this hook, two tests in the same file
// that both render the same component get two copies in the DOM
// and `screen.getByRole(...)` throws "found multiple elements".
afterEach(() => {
  cleanup();
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Tauri's `getVersion` lives in `@tauri-apps/api/app` — stubbed so
// the version-related stores work in tests without a real Tauri
// runtime. Individual tests can still override via
// `vi.mocked(getVersion).mockResolvedValue("0.0.5")`.
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("test"),
}));

// Tauri's updater plugin is dynamically imported in `updaterStore`,
// so test files that exercise it use `vi.mock("@tauri-apps/plugin-updater")`
// inline rather than this global setup.
