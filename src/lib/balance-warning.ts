// Balance-staleness warning logic, extracted from RiderCard.svelte
// so it's unit-testable without a Svelte component-mount round-trip.
//
// The contract: given a wallet address, the last-successful-fetch
// Unix-ms timestamp, and "now" in Unix-ms, return either null
// (healthy, hide the row) or `{ label }` with a human string for
// the stale row. v0.0.3 reshape made this error-only — we only
// surface a warning when ≥ 5 minutes have passed without a fresh
// fetch (something is actually wrong: Sui RPC throttling, network
// drop, etc.).

export interface BalanceWarning {
  label: string;
}

/** Decide whether to show the stale-balance warning for a rider.
 *
 *  Returns `null` when:
 *    - The rider has no wallet address set (nothing to fetch).
 *    - No successful fetch has happened yet (first-load — we'd
 *      rather show nothing than a confusing "never fetched"
 *      message during the cold-start window).
 *    - The most recent fetch is < 5 minutes old (healthy: the
 *      displayed figures are still fresh, no row needed).
 *    - The fetched timestamp is in the future relative to `now`
 *      (clock skew / NTP correction across the host — treat as
 *      "we can't trust the age", fail open).
 *
 *  Returns `{ label }` when ≥ 5 minutes have elapsed without a
 *  fresh fetch. Label uses minutes for < 1 h, hours after that.
 */
export function balanceWarning(
  walletAddress: string | null,
  walletBalanceFetchedAt: number | null,
  nowMs: number,
): BalanceWarning | null {
  if (!walletAddress) return null;
  if (walletBalanceFetchedAt == null) return null;
  const ageMs = nowMs - walletBalanceFetchedAt;
  if (ageMs < 0) return null;
  const ageMinutes = Math.floor(ageMs / 1000 / 60);
  if (ageMinutes < 5) return null;
  if (ageMinutes < 60) {
    return { label: `Balance stale · ${ageMinutes}m old` };
  }
  const ageHours = Math.floor(ageMinutes / 60);
  return { label: `Balance stale · ${ageHours}h old` };
}
