// Shared TypeScript types. These mirror the Rust models in
// src-tauri/src/rider.rs — keep them in sync.

export type RiderStatus =
  | "stopped"
  | "starting"
  | "running"
  | "error"
  /** The rider's Sandboxie box is no longer in Sandboxie.ini — the user
   *  (or another tool) deleted it externally. The rider record is now
   *  orphaned; the UI surfaces a "Sandbox missing" badge and a one-click
   *  Remove action that bypasses the normal archive-then-delete flow. */
  | "missing";

export interface Rider {
  /** Stable internal id, e.g. "frontier-1" */
  id: string;
  /** Display name shown in the UI */
  name: string;
  /** Sandboxie box name, e.g. "Frontier1" */
  sandbox: string;
  /** Per-rider Chromium `--user-data-dir`. Browser-agnostic; today
   *  Bifrost bundles Brave but the dir layout is standard Chromium. */
  browserProfileDir: string;
  /** Optional Sui wallet address once known (read-only display) */
  walletAddress: string | null;
  /** Optional last-known SUI gas-token balance, pre-formatted */
  walletBalance: string | null;
  /** Optional last-known EVE token balance (the player-facing currency),
   *  pre-formatted as a decimal string */
  eveBalance: string | null;
  /** Current lifecycle state */
  status: RiderStatus;
  /** Optional border / theme colour as #RRGGBB */
  accent: string;
  /** Archived riders are hidden from the Managed list and shown under
   *  Archived. Their sandboxes are preserved and can be restored. */
  archived: boolean;
  /** True once Bifrost has successfully launched the game in this rider's
   *  sandbox at least once. Used to suppress the first-launch hint ribbon. */
  launchedAtLeastOnce: boolean;
  /** Unix milliseconds at which `walletBalance` / `eveBalance` were last
   *  successfully refreshed from the Sui RPC. `null` until the first
   *  refresh succeeds. RiderCard reads this to surface a staleness
   *  badge — "Updated 5m ago" / "Stale" — so the user can tell at a
   *  glance whether the figures on screen are current or were last
   *  fetched 20 min ago when the network was up. */
  walletBalanceFetchedAt: number | null;
}

/** A single ecosystem app the user can launch into a rider's browser. */
export interface CompanionSite {
  name: string;
  /** 1–2 char monogram for the icon tile. */
  icon: string;
  url: string;
  /** True for sites bundled with Bifrost (cannot be removed, but can be
   *  disabled). */
  builtin: boolean;
  /** True when the user has hidden this site from the per-rider Apps
   *  row. Site stays in the config so re-enabling restores it. */
  disabled: boolean;
}

export interface BifrostConfig {
  /** Path to Sandboxie-Plus install. null = autodetect */
  sandboxiePath: string | null;
  /** Path to the EVE Frontier game executable. null = autodetect */
  frontierExe: string | null;
  /** Local directory where per-rider Chrome profiles live */
  ridersDir: string;
  /** Whether to auto-launch all enabled riders on app open */
  launchAllOnStart: boolean;
  /** Ordered list of ecosystem apps, both built-in and user-added. */
  companionSites: CompanionSite[];
  /** UI text-scale factor driving the `--text-scale` CSS variable on
   *  `:root`. Rem-based type sizes scale together; pixel-based
   *  chrome (button hit-targets, gaps) stays fixed — desktop-app
   *  shape, not browser-zoom shape. Settings exposes presets at 0.9
   *  (Compact), 1.0 (Default), and 1.15 (Comfortable); persisted as
   *  the raw float for forward compat with future keyboard-driven
   *  steps. */
  uiZoom: number;
  /** Preferred rider-card column count in the roster grid. `0` = auto
   *  (responsive — as many cards as the window can hold); `2` and
   *  `3` are explicit overrides surfaced under Settings › Display
   *  for multiboxers who want a locked layout. */
  rosterColumns: number;
  /** Last user-chosen window width in CSS pixels, captured while the
   *  user was in Auto roster mode. `null` for fresh installs or for
   *  users who have never tweaked the window in Auto. Restored on
   *  cold start so a 1600 × 1000 Auto layout reopens at exactly
   *  that geometry. */
  rosterWindowWidth: number | null;
  /** Last user-chosen window height in CSS pixels. Paired with
   *  `rosterWindowWidth` — both are written together by the
   *  debounced resize listener, so they never desync. */
  rosterWindowHeight: number | null;
}

/** EVE Vault extension install + update state. */
export interface EveVaultStatus {
  latestVersion: string | null;
  installedVersion: string | null;
  installDir: string | null;
  updateAvailable: boolean;
  /** Set when the GitHub fetch failed (rate-limit, network…). */
  latestError: string | null;
}

/** Portable Chromium install + update state. */
export interface ChromiumStatus {
  latestVersion: string | null;
  installedVersion: string | null;
  installDir: string | null;
  chromeExe: string | null;
  updateAvailable: boolean;
  latestSizeBytes: number | null;
  /** Set when the GitHub fetch failed (rate-limit, network…). */
  latestError: string | null;
}

/** Which Sandboxie variant is/was selected. Both ship from the same
 *  GitHub repo and share the kernel driver + CLI surface — they
 *  differ only in UI (Plus = modern Qt, Classic = legacy MFC). */
export type SandboxieVariant = "plus" | "classic";

/** Sandboxie installer state. Differs from ChromiumStatus in that
 *  Sandboxie can't be made portable (kernel driver), so we additionally
 *  track whether the binary is `detected` on disk (regardless of whether
 *  Bifrost installed it or the user did manually), and which variant is
 *  on disk (Plus vs Classic). */
export interface SandboxieInstallerStatus {
  /** Variant currently detected on disk. Null if nothing's installed. */
  installedVariant: SandboxieVariant | null;
  /** Tag Bifrost last installed via the in-app installer. Null if the
   *  user installed externally or not at all. */
  installedVersion: string | null;
  /** True when Sandboxie binaries are present on the host, regardless
   *  of which version or who installed them. */
  detected: boolean;
  /** Latest Plus release tag on GitHub. */
  latestPlusVersion: string | null;
  /** Latest Classic release tag on GitHub. */
  latestClassicVersion: string | null;
  updateAvailable: boolean;
  latestSizeBytes: number | null;
  /** Set when the GitHub fetch failed (rate-limit, network…). */
  latestError: string | null;
}

export interface AppStatus {
  sandboxieInstalled: boolean;
  frontierFound: boolean;
}

/** A Sandboxie box that exists on disk but isn't yet managed by Bifrost. */
export interface DiscoveredBox {
  name: string;
  configLevel: string | null;
  borderColor: string | null;
}
