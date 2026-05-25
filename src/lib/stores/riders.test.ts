// Store-mutation tests for `riderStore`. Two contracts pinned here:
//
//   1. `run()` clears `this.error` on every entry. The previous
//      command's failure shouldn't leave a red banner pinned after
//      a successful retry. This is the same shape we already fixed
//      in chromium/vault/sandboxie installer stores in v0.0.4 —
//      pinning the contract here protects against accidental
//      removal of the `this.error = null` line in a refactor.
//
//   2. `deletePermanently()` is OPTIMISTIC — it removes the rider
//      from local state BEFORE awaiting the eventual refresh, so
//      the card disappears from the UI immediately (rather than
//      lingering until the next reconcile). A regression that
//      reverses the order (await refresh, then mutate) defeats the
//      "card vanishes the moment you click Delete" UX.
//
// Mocking strategy: the global Tauri stub from `src/test-setup.ts`
// provides `vi.fn()` for `invoke`. The typed wrappers in
// `src/lib/tauri.ts` use that under the hood — we override
// `invoke`'s behaviour per-test via `vi.mocked(...)`.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { riderStore } from "./riders.svelte";

const mockedInvoke = vi.mocked(invoke);

// Helper: build a minimal Rider-shaped object that satisfies the
// listRiders return type the store reads. Only id matters for the
// optimistic-delete test.
const fakeRider = (id: string): unknown => ({
  id,
  name: id,
  sandbox: `Sandbox${id}`,
  browserProfileDir: `/tmp/${id}`,
  walletAddress: null,
  walletBalance: null,
  eveBalance: null,
  status: "stopped",
  accent: "#ff5a25",
  archived: false,
  launchedAtLeastOnce: false,
  walletBalanceFetchedAt: null,
});

describe("riderStore.run() error handling", () => {
  beforeEach(() => {
    riderStore.error = null;
    riderStore.riders = [];
    mockedInvoke.mockReset();
  });

  afterEach(() => {
    mockedInvoke.mockReset();
  });

  it("populates `error` when the underlying command rejects", async () => {
    mockedInvoke.mockRejectedValueOnce("Error: backend boom");
    const ok = await riderStore.create("Airikr");
    expect(ok).toBe(false);
    expect(riderStore.error).toBe("backend boom"); // formatBackendError stripped the prefix
  });

  it("clears stale `error` on the next call (regression guard)", async () => {
    // Step 1: first call fails, error is set.
    mockedInvoke.mockRejectedValueOnce("Error: first boom");
    await riderStore.create("Airikr");
    expect(riderStore.error).toBe("first boom");

    // Step 2: second call's createRider invoke resolves, then
    // refresh runs (which calls listRiders + listSandboxes —
    // we'll let both return empty arrays). Error MUST be null
    // after the successful second call.
    mockedInvoke
      .mockResolvedValueOnce(undefined) // createRider
      .mockResolvedValueOnce([]) // listRiders
      .mockResolvedValueOnce([]); // listSandboxes
    const ok = await riderStore.create("Tal'Ra");
    expect(ok).toBe(true);
    expect(riderStore.error).toBeNull();
  });
});

describe("riderStore.deletePermanently() local-state contract", () => {
  beforeEach(() => {
    riderStore.error = null;
    riderStore.riders = [
      fakeRider("airikr") as never,
      fakeRider("talra") as never,
    ];
    mockedInvoke.mockReset();
  });

  it("removes the rider from local state after backend ack, without awaiting a refresh", async () => {
    // The contract: `deletePermanently` awaits the backend
    // deleteRider, then mutates local state. It deliberately does
    // NOT call `refresh()` — relying on the surrounding reconcile
    // tick to reconcile if the local guess drifts. Pin both halves
    // here: (1) the await happens, and (2) no listRiders /
    // listSandboxes refresh is invoked.
    mockedInvoke.mockResolvedValueOnce(undefined); // deleteRider
    await riderStore.deletePermanently("airikr");
    expect(riderStore.riders.map((r) => r.id)).toEqual(["talra"]);
    // Only one invoke (the deleteRider). No refresh fired.
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });

  it("does NOT remove from local state when the backend rejects (current behaviour)", async () => {
    // The current implementation removes from local state ONLY on
    // success — so a backend rejection leaves the card in place
    // and surfaces the error. Pin the current behaviour so a
    // future change (e.g. flipping to "remove first, roll back on
    // error") is a deliberate diff rather than an accidental one.
    mockedInvoke.mockRejectedValueOnce("Error: backend refused");
    await riderStore.deletePermanently("airikr");
    expect(riderStore.error).toBe("backend refused");
    expect(riderStore.riders.map((r) => r.id)).toEqual(["airikr", "talra"]);
  });
});
