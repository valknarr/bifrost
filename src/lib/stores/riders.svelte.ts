// Svelte 5 rune-based store for the rider list. Owns the in-memory copy of
// what the Rust backend reports, plus loading state.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import type { DiscoveredBox, Rider } from "../types";

class RiderStore {
  riders = $state<Rider[]>([]);
  discovered = $state<DiscoveredBox[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  /** True while the cold-start `bootstrap()` is in its slow phase —
   *  i.e. the cached roster is already on screen but
   *  `reconcile_riders` hasn't returned yet, so per-rider statuses
   *  may be stale.
   *
   *  Drives two UI affordances:
   *    1. A thin "Validating sandboxes…" indicator above the roster
   *       so the user understands why they're seeing a moment of
   *       UI quiet.
   *    2. Launch / Stop buttons disable while syncing — clicking
   *       Launch on a cached-stopped rider that's actually running
   *       would spawn a second game instance, which is hard to
   *       recover from. The brief disable is a cheap safety net.
   *
   *  Only flips during the cold-start `bootstrap()`; the 30 s
   *  background `reconcile()` tick deliberately leaves it false so
   *  the UI doesn't twitch every half-minute. */
  syncing = $state(false);

  /** Run a backend mutation, clear the previous error on entry, refresh on
   *  success, capture the error on failure. Avoids the "stale error stays
   *  visible after a later successful action" bug. */
  private async run(fn: () => Promise<unknown>): Promise<boolean> {
    this.error = null;
    try {
      await fn();
      await this.refresh();
      return true;
    } catch (e) {
      this.error = formatBackendError(e);
      return false;
    }
  }

  async refresh() {
    this.loading = true;
    try {
      const [riders, discovered] = await Promise.all([
        api.listRiders(),
        api.listSandboxes(),
      ]);
      this.riders = riders;
      this.discovered = discovered;
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.loading = false;
    }
  }

  create = (name: string) => this.run(() => api.createRider(name));
  start = (id: string) => this.run(() => api.startRider(id));
  stop = (id: string) => this.run(() => api.stopRider(id));
  archive = (id: string) => this.run(() => api.archiveRider(id));
  restore = (id: string) => this.run(() => api.restoreRider(id));
  setWallet = (id: string, address: string) =>
    this.run(() => api.setRiderWallet(id, address));
  setAccent = (id: string, accent: string) =>
    this.run(() => api.setRiderAccent(id, accent));
  reconcile = () => this.run(() => api.reconcileRiders());
  adopt = (boxName: string, displayName: string) =>
    this.run(() => api.adoptSandbox(boxName, displayName));

  /** Cold-start bootstrap. Shows the cached roster from `riders.json`
   *  immediately (fast — just an in-memory `listRiders` call), then
   *  validates statuses against Sandboxie's actual runtime state in
   *  the background.
   *
   *  Splitting these into two phases is the whole point: before this
   *  existed, `RidersView` awaited `reconcile_riders` (one Start.exe
   *  shellout per rider + tasklist parsing + a Sui balance refresh)
   *  before rendering anything, leaving the user staring at an empty
   *  panel for 1–3 s on every app open. Now the cards appear in the
   *  first frame; only the per-rider status badges are briefly
   *  authoritative-on-disk rather than authoritative-right-now. */
  async bootstrap() {
    await this.refresh();
    this.syncing = true;
    try {
      await api.reconcileRiders();
      await this.refresh();
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.syncing = false;
    }
  }

  /** Permanently delete an unmanaged Sandboxie box (one that's in the
   *  Discovered list, not associated with a Bifrost rider). Re-runs the
   *  full reconcile afterwards so the box disappears from the
   *  Discovered grid on success. */
  async deleteSandbox(boxName: string) {
    this.error = null;
    try {
      await api.deleteSandbox(boxName);
      this.discovered = this.discovered.filter((b) => b.name !== boxName);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  /** Permanently delete a Bifrost-managed rider (sandbox + record).
   *  Bypasses `run()` deliberately: optimistic local removal of the
   *  rider before a full reconcile so the card disappears
   *  immediately, even if the subsequent listRiders refetch takes a
   *  moment (which it can on disks with many Sandboxie boxes). The
   *  reconcile loop (`RidersView` interval) will recover state if
   *  the backend disagrees with our local guess. */
  async deletePermanently(id: string) {
    this.error = null;
    try {
      await api.deleteRider(id);
      this.riders = this.riders.filter((p) => p.id !== id);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  /** Manually dismiss the error banner. */
  clearError() {
    this.error = null;
  }
}

export const riderStore = new RiderStore();
