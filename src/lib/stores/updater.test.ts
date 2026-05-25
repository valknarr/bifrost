// State-machine tests for `updaterStore.check()`. The store's
// `lastResult` field drives the Settings → About panel's text
// (added in v0.0.3, refined in v0.0.7). Three terminal states:
//
//   * `"up-to-date"` — plugin returned `null`, no update.
//   * `"available"` — plugin returned an `Update` object.
//   * `"error"`     — plugin threw (no network, pubkey wrong, etc.).
//
// Pin all three so a refactor that wires `lastResult` to the wrong
// branch can't slip past CI — the About panel reads this directly
// to render "✓ Up to date" vs "Update available" vs the error
// pill.
//
// Mocking strategy: the updater store calls `await import(
// "@tauri-apps/plugin-updater")` lazily, so we `vi.mock` the
// plugin module to control `check`'s return value. Force-true to
// `updaterStore.check()` bypasses the DEV-skip guard since
// vitest sets `import.meta.env.DEV`.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { updaterStore } from "./updater.svelte";

// Mock the dynamic-imported plugin BEFORE the store reads it. The
// store does `const { check } = await import("@tauri-apps/plugin-
// updater")` inside check(); this vi.mock factory makes that
// resolve to our controllable stub.
const mockCheck = vi.fn();
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: () => mockCheck(),
}));

describe("updaterStore.check() lastResult state machine", () => {
  beforeEach(() => {
    updaterStore.available = null;
    updaterStore.error = null;
    updaterStore.lastResult = null;
    updaterStore.checking = false;
    mockCheck.mockReset();
  });

  afterEach(() => {
    mockCheck.mockReset();
  });

  it("transitions to `up-to-date` when the plugin returns null", async () => {
    mockCheck.mockResolvedValueOnce(null);
    await updaterStore.check(true);
    expect(updaterStore.lastResult).toBe("up-to-date");
    expect(updaterStore.available).toBeNull();
    expect(updaterStore.error).toBeNull();
    expect(updaterStore.checking).toBe(false);
  });

  it("transitions to `available` and stores the Update when one is returned", async () => {
    const fakeUpdate = { version: "0.0.9" } as never;
    mockCheck.mockResolvedValueOnce(fakeUpdate);
    await updaterStore.check(true);
    expect(updaterStore.lastResult).toBe("available");
    // Svelte 5's $state wraps assigned values in a Proxy, so a
    // reference-equality check (toBe) on the stored object fails.
    // Probe through the derived getter instead — it returns the
    // proxy's `version` field via `available?.version`.
    expect(updaterStore.newVersion).toBe("0.0.9");
    expect(updaterStore.available).not.toBeNull();
    expect(updaterStore.error).toBeNull();
  });

  it("transitions to `error` and captures the message when the plugin throws", async () => {
    mockCheck.mockRejectedValueOnce(new Error("offline"));
    await updaterStore.check(true);
    expect(updaterStore.lastResult).toBe("error");
    expect(updaterStore.available).toBeNull();
    expect(updaterStore.error).toContain("offline");
  });

  it("clears `lastResult` and `error` on entry of the next check", async () => {
    // Step 1: error state established.
    mockCheck.mockRejectedValueOnce(new Error("offline"));
    await updaterStore.check(true);
    expect(updaterStore.lastResult).toBe("error");

    // Step 2: successful check; lastResult flips to up-to-date,
    // error clears.
    mockCheck.mockResolvedValueOnce(null);
    await updaterStore.check(true);
    expect(updaterStore.lastResult).toBe("up-to-date");
    expect(updaterStore.error).toBeNull();
  });

  it("no-ops when a check is already in flight", async () => {
    // Hold the first check open with a never-resolving promise so
    // we can race a second call against it. The store's guard is
    // `if (this.checking) return;` at the top of check().
    mockCheck.mockReturnValueOnce(new Promise(() => {}));
    const firstCheck = updaterStore.check(true);

    // The first call has to await `import("@tauri-apps/plugin-
    // updater")` (a real dynamic import even when vi.mocked)
    // before it reaches `await check()`. Yield enough microtasks
    // for that import to settle and the never-resolving
    // mockCheck() to be invoked once. setTimeout(0) is the
    // cleanest "let the queue drain" hook in vitest's jsdom env.
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(updaterStore.checking).toBe(true);
    expect(mockCheck).toHaveBeenCalledTimes(1);

    // A second call while checking should early-return and NOT
    // invoke the plugin a second time.
    await updaterStore.check(true);
    expect(mockCheck).toHaveBeenCalledTimes(1);

    // Cleanup: orphan the first promise (it never resolves).
    void firstCheck;
  });
});
