// Typed wrappers around Tauri's `invoke()` so view code doesn't need to know
// command names. Each function maps 1:1 to a #[tauri::command] in lib.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  AppStatus,
  BifrostConfig,
  ChromiumStatus,
  DiscoveredBox,
  EveVaultStatus,
  Pilot,
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

  // --- pilots ---
  listPilots: () => invoke<Pilot[]>("list_pilots"),
  createPilot: (name: string) => invoke<Pilot>("create_pilot", { name }),
  archivePilot: (id: string) => invoke<void>("archive_pilot", { id }),
  restorePilot: (id: string) => invoke<void>("restore_pilot", { id }),
  deletePilot: (id: string) => invoke<void>("delete_pilot", { id }),
  setPilotAccent: (id: string, accent: string) =>
    invoke<void>("set_pilot_accent", { id, accent }),
  getAccentPalette: () => invoke<string[]>("get_accent_palette"),
  startPilot: (id: string) => invoke<void>("start_pilot", { id }),
  stopPilot: (id: string) => invoke<void>("stop_pilot", { id }),
  setPilotWallet: (id: string, address: string) =>
    invoke<Pilot>("set_pilot_wallet", { id, address }),
  reconcilePilots: () => invoke<void>("reconcile_pilots"),

  // --- sandboxie ---
  detectSandboxie: () => invoke<string | null>("detect_sandboxie"),
  listSandboxes: () => invoke<DiscoveredBox[]>("list_sandboxes"),
  adoptSandbox: (boxName: string, displayName: string) =>
    invoke<Pilot>("adopt_sandbox", { boxName, displayName }),
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
  openPilotBrowser: (id: string) =>
    invoke<void>("open_pilot_browser", { id }),
  openPilotApp: (id: string, url: string) =>
    invoke<void>("open_pilot_app", { id, url }),
  addCompanionSite: (name: string, url: string, icon?: string) =>
    invoke<BifrostConfig>("add_companion_site", { name, url, icon }),
  removeCompanionSite: (url: string) =>
    invoke<BifrostConfig>("remove_companion_site", { url }),
  setCompanionSiteDisabled: (url: string, disabled: boolean) =>
    invoke<BifrostConfig>("set_companion_site_disabled", { url, disabled }),

  /** Resolve a companion-site URL to a `data:image/png;base64,â€¦`
   *  string, or null when no favicon could be retrieved. Rust handles
   *  caching + negative-caching, so calling this freely from any
   *  effect is cheap after the first hit. */
  getFavicon: (url: string) => invoke<string | null>("get_favicon", { url }),
} as const;
