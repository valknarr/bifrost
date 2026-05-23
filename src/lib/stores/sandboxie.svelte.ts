// Sandboxie-Plus installer status store. Mirrors chromiumStore / vaultStore
// shape so the Settings UI can render Install / Update / Reinstall
// buttons in parity with the other two managed components.
//
// Sandboxie-Plus differs in one important way: it can't be portable
// (kernel driver). The store still exposes `detected` to distinguish
// "binary is on disk somewhere" from "Bifrost installed a specific tag",
// because users who installed Sandboxie themselves should still see
// "Detected" even without a Bifrost-tracked version.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import { statusStore } from "./status.svelte";
import type { SandboxieInstallerStatus, SandboxieVariant } from "../types";

class SandboxieStore {
  status = $state<SandboxieInstallerStatus | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  async refresh(force = false) {
    try {
      this.status = await api.getSandboxieInstallerStatus(force);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  /** Install Sandboxie. Defaults to the Plus variant (the recommended
   *  modern UI); pass "classic" to install the legacy MFC build. Only
   *  one variant can be installed at a time — Bifrost does NOT switch
   *  variants automatically (uninstall first, then install the other). */
  async install(variant: SandboxieVariant = "plus") {
    this.busy = true;
    this.error = null;
    try {
      await api.installSandboxie(variant);
      // Re-probe both this store AND the host-detection store so the
      // banner + tab badge update immediately.
      await Promise.all([this.refresh(), statusStore.refresh()]);
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
      await api.uninstallSandboxie();
      // The host-detection store needs to refresh too so the SetupBanner
      // reappears (Sandboxie is required, removing it should immediately
      // surface as a missing dependency).
      await Promise.all([this.refresh(), statusStore.refresh()]);
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

export const sandboxieStore = new SandboxieStore();
