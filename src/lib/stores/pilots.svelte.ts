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
