// Portable Chromium status store. Mirrors vaultStore for parity: the
// Settings UI manages both alongside each other under the wallet
// integration umbrella.

import { api } from "../tauri";
import { formatBackendError } from "../error";
import type { ChromiumStatus } from "../types";

class ChromiumStore {
  status = $state<ChromiumStatus | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  /** True when a portable Chromium build is on disk and ready to use. */
  get ready(): boolean {
    return !!(this.status?.installedVersion && this.status?.chromeExe);
  }

  async refresh(force = false) {
    // Clear any stale error from a previous install / uninstall
    // failure on entry — once we successfully refresh the status,
    // an old red banner from a previous click shouldn't stay
    // pinned. Same shape as `status.svelte.ts::refresh`.
    this.error = null;
    try {
      this.status = await api.getChromiumStatus(force);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  async install() {
    this.busy = true;
    this.error = null;
    try {
      await api.installChromium();
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
      await api.uninstallChromium();
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

export const chromiumStore = new ChromiumStore();
