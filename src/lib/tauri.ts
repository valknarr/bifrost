// Typed wrappers around Tauri's `invoke()` so view code doesn't need to know
// command names. Each function maps 1:1 to a #[tauri::command] in lib.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  BifrostConfig,
  ChromiumStatus,
  DiscoveredBox,
  EveVaultStatus,
  Rider,
  SandboxieInstallerStatus,
  SandboxieVariant,
} from "./types";

export const api = {
  // --- system / config ---
  getStatus: () => invoke<AppStatus>("get_status"),
  getConfig: () => invoke<BifrostConfig>("get_config"),
  setConfig: (config: BifrostConfig) =>
    invoke<void>("set_config", { config }),
  setUiZoom: (zoom: number) => invoke<BifrostConfig>("set_ui_zoom", { zoom }),
  setRosterColumns: (columns: number) =>
    invoke<BifrostConfig>("set_roster_columns", { columns }),
  setRosterWindowSize: (width: number, height: number) =>
    invoke<BifrostConfig>("set_roster_window_size", { width, height }),

  // --- riders ---
  listRiders: () => invoke<Rider[]>("list_riders"),
  createRider: (name: string) => invoke<Rider>("create_rider", { name }),
  archiveRider: (id: string) => invoke<void>("archive_rider", { id }),
  restoreRider: (id: string) => invoke<void>("restore_rider", { id }),
  deleteRider: (id: string) => invoke<void>("delete_rider", { id }),
  setRiderAccent: (id: string, accent: string) =>
    invoke<void>("set_rider_accent", { id, accent }),
  getAccentPalette: () => invoke<string[]>("get_accent_palette"),
  startRider: (id: string) => invoke<void>("start_rider", { id }),
  stopRider: (id: string) => invoke<void>("stop_rider", { id }),
  setRiderWallet: (id: string, address: string) =>
    invoke<Rider>("set_rider_wallet", { id, address }),
  reconcileRiders: () => invoke<void>("reconcile_riders"),

  // --- sandboxie ---
  detectSandboxie: () => invoke<string | null>("detect_sandboxie"),
  listSandboxes: () => invoke<DiscoveredBox[]>("list_sandboxes"),
  adoptSandbox: (boxName: string, displayName: string) =>
    invoke<Rider>("adopt_sandbox", { boxName, displayName }),
  deleteSandbox: (boxName: string) =>
    invoke<void>("delete_sandbox", { boxName }),

  // --- EVE Vault integration ---
  //
  // `forceRefresh` (default false) controls whether the backend bypasses
  // its 30-min release cache. Passive panel mounts pass false so we
  // don't burn through GitHub's 60-req/hr unauthenticated quota; the
  // "Check for updates" buttons pass true.
  getEveVaultStatus: (forceRefresh = false) =>
    invoke<EveVaultStatus>("get_evevault_status", { forceRefresh }),
  installEveVault: () => invoke<void>("install_evevault"),
  getChromiumStatus: (forceRefresh = false) =>
    invoke<ChromiumStatus>("get_chromium_status", { forceRefresh }),
  installChromium: () => invoke<void>("install_chromium"),
  uninstallChromium: () => invoke<void>("uninstall_chromium"),
  uninstallEvevault: () => invoke<void>("uninstall_evevault"),
  getSandboxieInstallerStatus: (forceRefresh = false) =>
    invoke<SandboxieInstallerStatus>("get_sandboxie_installer_status", {
      forceRefresh,
    }),
  installSandboxie: (variant: SandboxieVariant = "plus") =>
    invoke<void>("install_sandboxie", { variant }),
  uninstallSandboxie: () => invoke<void>("uninstall_sandboxie"),
  openRiderBrowser: (id: string) =>
    invoke<void>("open_rider_browser", { id }),
  openRiderApp: (id: string, url: string) =>
    invoke<void>("open_rider_app", { id, url }),
  addCompanionSite: (name: string, url: string, icon?: string) =>
    invoke<BifrostConfig>("add_companion_site", { name, url, icon }),
  removeCompanionSite: (url: string) =>
    invoke<BifrostConfig>("remove_companion_site", { url }),
  setCompanionSiteDisabled: (url: string, disabled: boolean) =>
    invoke<BifrostConfig>("set_companion_site_disabled", { url, disabled }),

  /** Resolve a companion-site URL to a `data:image/png;base64,…`
   *  string, or null when no favicon could be retrieved. Rust handles
   *  caching + negative-caching, so calling this freely from any
   *  effect is cheap after the first hit. */
  getFavicon: (url: string) => invoke<string | null>("get_favicon", { url }),
} as const;
