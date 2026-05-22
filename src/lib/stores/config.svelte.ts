// App-level configuration store. Shared by SettingsView (writes) and
// any component that needs to read config-derived state (PilotCard reads
// the companion-sites list).

import { api } from "../tauri";
import { formatBackendError } from "../error";
import type { BridgeConfig, CompanionSite } from "../types";
import { applyZoom } from "../zoom";

class ConfigStore {
  config = $state<BridgeConfig | null>(null);
  error = $state<string | null>(null);

  /** Full list, including disabled entries — used by SettingsView so the
   *  user can toggle disabled state. */
  get sites(): CompanionSite[] {
    return this.config?.companionSites ?? [];
  }

  /** Only enabled sites — what PilotCard's Apps row consumes. */
  get enabledSites(): CompanionSite[] {
    return this.sites.filter((s) => !s.disabled);
  }

  async refresh() {
    try {
      this.config = await api.getConfig();
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  async addSite(name: string, url: string, icon?: string) {
    this.error = null;
    try {
      this.config = await api.addCompanionSite(name, url, icon);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  async removeSite(url: string) {
    this.error = null;
    try {
      this.config = await api.removeCompanionSite(url);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  async setSiteDisabled(url: string, disabled: boolean) {
    this.error = null;
    try {
      this.config = await api.setCompanionSiteDisabled(url, disabled);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }

  /** Apply a zoom factor to the running webview AND persist it for
   *  the next launch. Order matters: apply first so the user sees
   *  the change instantly, persist second so a backend save error
   *  doesn't leave the UI mismatched with the saved config. Both
   *  steps surface their errors to `this.error` — the Settings
   *  picker is the user-visible caller and shows them inline. */
  async setUiZoom(zoom: number) {
    this.error = null;
    try {
      await applyZoom(zoom);
    } catch (e) {
      this.error =
        "Couldn't change zoom — the webview permission may be missing. " +
        formatBackendError(e);
      return;
    }
    try {
      this.config = await api.setUiZoom(zoom);
    } catch (e) {
      this.error = formatBackendError(e);
    }
  }
}

export const configStore = new ConfigStore();
