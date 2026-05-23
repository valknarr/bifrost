// EVE Vault extension state. Lives at module scope so any component
// can subscribe.
//
// Note: there's no longer an explicit "enable integration" toggle. The
// wallet integration is automatically active whenever both Brave and
// the EVE Vault extension are installed — see `integrationReady()`
// below, which the RiderCard reads to decide whether to render the
// per-rider Apps row.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import { chromiumStore } from "./chromium.svelte";
import type { EveVaultStatus } from "../types";

class VaultStore {
  status = $state<EveVaultStatus | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  /** True when the extension is installed locally. Combine with
   *  `chromiumStore.ready` via [`integrationReady`] for the full
   *  "is the wallet flow usable?" check. */
  get ready(): boolean {
    return !!this.status?.installedVersion;
  }

  async refresh(force = false) {
    try {
      this.status = await api.getEveVaultStatus(force);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  async install() {
    this.busy = true;
    this.error = null;
    try {
      await api.installEveVault();
      await this.refresh();
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.busy = false;
    }
  }

  async uninstall() {
    this.busy = true;
    this.error = null;
    try {
      await api.uninstallEvevault();
      await this.refresh();
    } catch (e) {
      this.error = formatBackendError(e);
    } finally {
      this.busy = false;
    }
  }

  clearError() {
    this.error = null;
  }
}

export const vaultStore = new VaultStore();

/**
 * The wallet integration is "ready" — meaning Bifrost can actually
 * launch a per-rider browser with the EVE Vault extension preloaded —
 * only when BOTH the portable browser and the extension itself are
 * installed. Component code reads this instead of any single flag so
 * the UI auto-syncs the moment either dependency lands or is removed.
 */
export function integrationReady(): boolean {
  return chromiumStore.ready && vaultStore.ready;
}
