// Svelte 5 rune-based store for the pilot list. Owns the in-memory copy of
// what the Rust backend reports, plus loading state.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import type { DiscoveredBox, Pilot } from "../types";

class PilotStore {
  pilots = $state<Pilot[]>([]);
  discovered = $state<DiscoveredBox[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);

  /** True while the cold-start `bootstrap()` is in its slow phase —
   *  i.e. the cached roster is already on screen but
   *  `reconcile_pilots` hasn't returned yet, so per-pilot statuses
   *  may be stale.
   *
   *  Drives two UI affordances:
   *    1. A thin "Validating sandboxes…" indicator above the roster
   *       so the user understands why they're seeing a moment of
   *       UI quiet.
   *    2. Launch / Stop buttons disable while syncing — clicking
   *       Launch on a cached-stopped pilot that's actually running
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
      const [pilots, discovered] = await Promise.all([
        api.listPilots(),
        api.listSandboxes(),
      ]);
      this.pilots = pilots;
      this.discovered = discovered;
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.loading = false;
    }
  }

  create = (name: string) => this.run(() => api.createPilot(name));
  start = (id: string) => this.run(() => api.startPilot(id));
  stop = (id: string) => this.run(() => api.stopPilot(id));
  archive = (id: string) => this.run(() => api.archivePilot(id));
  restore = (id: string) => this.run(() => api.restorePilot(id));
  setWallet = (id: string, address: string) =>
    this.run(() => api.setPilotWallet(id, address));
  setAccent = (id: string, accent: string) =>
    this.run(() => api.setPilotAccent(id, accent));
  reconcile = () => this.run(() => api.reconcilePilots());
  adopt = (boxName: string, displayName: string) =>
    this.run(() => api.adoptSandbox(boxName, displayName));

  /** Cold-start bootstrap. Shows the cached roster from `pilots.json`
   *  immediately (fast — just an in-memory `listPilots` call), then
   *  validates statuses against Sandboxie's actual runtime state in
   *  the background.
   *
   *  Splitting these into two phases is the whole point: before this
   *  existed, `PilotsView` awaited `reconcile_pilots` (one Start.exe
   *  shellout per pilot + tasklist parsing + a Sui balance refresh)
   *  before rendering anything, leaving the user staring at an empty
   *  panel for 1–3 s on every app open. Now the cards appear in the
   *  first frame; only the per-pilot status badges are briefly
   *  authoritative-on-disk rather than authoritative-right-now. */
  async bootstrap() {
    await this.refresh();
    this.syncing = true;
    try {
      await api.reconcilePilots();
      await this.refresh();
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.syncing = false;
    }
  }

  /** Permanently delete an unmanaged Sandboxie box (one that's in the
   *  Discovered list, not associated with a Bifrost pilot). Re-runs the
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

  /** Permanently delete a Bifrost-managed pilot (sandbox + record).
   *  Bypasses `run()` deliberately: optimistic local removal of the
   *  pilot before a full reconcile so the card disappears
   *  immediately, even if the subsequent listPilots refetch takes a
   *  moment (which it can on disks with many Sandboxie boxes). The
   *  reconcile loop (`PilotsView` interval) will recover state if
   *  the backend disagrees with our local guess. */
  async deletePermanently(id: string) {
    this.error = null;
    try {
      await api.deletePilot(id);
      this.pilots = this.pilots.filter((p) => p.id !== id);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  /** Manually dismiss the error banner. */
  clearError() {
    this.error = null;
  }
}

export const pilotStore = new PilotStore();
