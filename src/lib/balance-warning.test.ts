// Boundary tests for `balanceWarning`. The 5-minute threshold is
// the load-bearing piece of v0.0.3's "error-only freshness" UX
// reshape — before that the row was always visible with "Updated
// Xm ago" chatter that caused minor card-height jitter every 15s.
// We want explicit tests at the seam so a future refactor that
// flips the comparator (`<` → `<=`, `>=` → `>`) is loud about it.
//
// Time scale: `now` and `walletBalanceFetchedAt` are Unix-ms (the
// shape `clockStore.now` and `rider.walletBalanceFetchedAt`
// already use), so test values are integers like 5 * 60 * 1000.

import { describe, expect, it } from "vitest";
import { balanceWarning } from "./balance-warning";

const NOW = 1_700_000_000_000; // arbitrary fixed reference instant
const MIN = 60_000;

describe("balanceWarning — null cases (healthy / not-applicable)", () => {
  it("returns null when there's no wallet address (nothing to fetch)", () => {
    expect(balanceWarning(null, NOW - 10 * MIN, NOW)).toBeNull();
    expect(balanceWarning("", NOW - 10 * MIN, NOW)).toBeNull();
  });

  it("returns null when no successful fetch has happened yet", () => {
    expect(balanceWarning("0xabc", null, NOW)).toBeNull();
  });

  it("returns null when the fetched timestamp is in the future (clock skew)", () => {
    // NTP correction during a session can push the host clock
    // backwards relative to a previously-stored timestamp. Fail
    // open rather than declare "stale" — the figure was fresh a
    // moment ago.
    expect(balanceWarning("0xabc", NOW + 60_000, NOW)).toBeNull();
  });
});

describe("balanceWarning — 5-minute boundary", () => {
  it("returns null at 4 min 59 s (just-fresh)", () => {
    const fetched = NOW - (5 * MIN - 1_000);
    expect(balanceWarning("0xabc", fetched, NOW)).toBeNull();
  });

  it("returns null at exactly 4 min — Math.floor keeps us under 5", () => {
    const fetched = NOW - 4 * MIN;
    expect(balanceWarning("0xabc", fetched, NOW)).toBeNull();
  });

  it("returns a warning at exactly 5 min — the seam", () => {
    const fetched = NOW - 5 * MIN;
    const result = balanceWarning("0xabc", fetched, NOW);
    expect(result).not.toBeNull();
    expect(result?.label).toBe("Balance stale · 5m old");
  });

  it("returns a warning at 5 min 30 s — still rounds to 5m", () => {
    const fetched = NOW - (5 * MIN + 30_000);
    expect(balanceWarning("0xabc", fetched, NOW)?.label).toBe(
      "Balance stale · 5m old",
    );
  });
});

describe("balanceWarning — labels at various ages", () => {
  it("uses minutes for 5..59 min ages", () => {
    expect(balanceWarning("0xabc", NOW - 12 * MIN, NOW)?.label).toBe(
      "Balance stale · 12m old",
    );
    expect(balanceWarning("0xabc", NOW - 59 * MIN, NOW)?.label).toBe(
      "Balance stale · 59m old",
    );
  });

  it("flips to hours at 60 min", () => {
    expect(balanceWarning("0xabc", NOW - 60 * MIN, NOW)?.label).toBe(
      "Balance stale · 1h old",
    );
    expect(balanceWarning("0xabc", NOW - 3 * 60 * MIN, NOW)?.label).toBe(
      "Balance stale · 3h old",
    );
  });

  it("floors the hour count (e.g. 1h 59m → 1h)", () => {
    const fetched = NOW - (119 * MIN);
    expect(balanceWarning("0xabc", fetched, NOW)?.label).toBe(
      "Balance stale · 1h old",
    );
  });
});
