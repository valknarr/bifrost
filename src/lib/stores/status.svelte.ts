// Shared host-detection state.
//
// Previously SettingsView fetched `get_status` directly on mount; now
// any view can subscribe so the Pilots view can surface a "setup
// required" banner without duplicating the call.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import type { AppStatus } from "../types";

class StatusStore {
  status = $state<AppStatus | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  /** True once `refresh()` has produced a status object (regardless of
   *  what's installed). Lets views distinguish "still probing" from
   *  "probe says nothing is installed". */
  get ready(): boolean {
    return this.status !== null;
  }

  /** Convenience: true when EITHER Sandboxie or the EVE Frontier client
   *  is missing. Used to decide whether to show the setup banner. */
  get missingAny(): boolean {
    if (!this.status) return false;
    return !this.status.sandboxieInstalled || !this.status.frontierFound;
  }

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      this.status = await api.getStatus();
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.loading = false;
    }
  }
}

export const statusStore = new StatusStore();
