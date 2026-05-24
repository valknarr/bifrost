# Changelog

All notable changes to Bifrost will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Nothing yet._

## [0.0.2] - 2026-05-24

### Security

- **Explicit Content Security Policy.** Bifrost's Tauri webview now
  runs under a deliberate CSP rather than `csp: null`. The Sui
  wallet integration makes the supply-chain attack class
  (compromised npm or Cargo dep silently exfiltrating addresses)
  worth closing: `connect-src 'self' ipc: https://ipc.localhost`
  means the webview cannot reach any host other than Tauri's IPC
  bridge, so address exfiltration via the frontend is blocked
  even if a dep is hijacked. Full policy + threat-model rationale
  in `SECURITY.md`.

## [0.0.1] - 2026-05-24

First public release. Bifrost orchestrates per-Rider sandboxed game
clients, browsers, and wallets for EVE Frontier multi-Rider play.
Source on GitHub; signed Windows installer attached to the GitHub
Release. Auto-updates via the in-app banner on subsequent launches.

### Added

- **Rider roster** — designate, launch, archive, restore, and delete EVE
  Frontier riders. Each rider is backed by a Sandboxie box and a Brave
  profile that survive across sessions. Boxes the user already has on
  disk surface in a "Discovered" section and can be adopted into Bifrost
  with one click.
- **In-app installers** for the three managed dependencies:
  - **Sandboxie** (Plus or Classic). Plus is the modern Qt UI and
    Bifrost's recommended default; Classic is the legacy MFC build on
    long-term support. Only one variant can be installed at a time
    (they share the kernel driver). Bifrost writes the variant + version
    it installed to a `.bifrost-installed-version` marker so Settings
    can report "up to date" / "update available" accurately.
  - **Portable Brave** (~180 MB). Bundled per-process so Bifrost doesn't
    fight a user's day-to-day browser. Quieted via
    `--disable-features` flags plus matching `Preferences` JSON keys —
    no Brave Rewards, no native crypto wallet popup, no Brave Talk /
    News / Leo / VPN, no "set as default" first-run nudges.
  - **EVE Vault** Chromium extension. Downloaded from upstream
    `evefrontier/evevault` releases, SHA-256 verified, side-loaded
    into every per-rider browser session.
- **Per-rider browser launch** — each rider opens its own Brave window
  inside its sandbox, with the EVE Vault extension preloaded and a
  small generated theme extension that tints the window frame with the
  rider's accent colour. Multiple riders can have their wallet sessions
  open simultaneously without overlapping.
- **Companion sites** — built-in EVE Map link and a custom-site
  manager. Each rider's card has an Apps row that opens these sites
  using the rider's wallet identity. Built-ins can be hidden but not
  removed; custom sites can be added and removed at will.
- **Sui mainnet wallet readout** — Bifrost reads SUI + EVE token
  balances for each rider's wallet via the public `suix_getBalance`
  JSON-RPC endpoint. Read-only — Bifrost never signs or submits a
  transaction.
- **Setup banner + dual-state HUD** — the Riders view warns when host
  dependencies are missing (Sandboxie not installed, EVE Frontier
  client not found) with one-click links into Settings. The top-right
  header shows a two-dot indicator: rider lifecycle (Online · N/N)
  plus system health (System / Setup).
- **EVE-Frontier-aligned visual language** — sharp rectangles, mono
  type, red-orange accent, `[#]`-bracketed window titles. Sparse
  procedural multi-galaxy background renders three slowly-spinning
  spiral galaxies that drift toward each other over many sessions
  (floored at 50 % of initial separation so they never actually
  merge). Rider cards carry corner brackets, an "energy" bar in the
  accent colour, and a halftone portrait placeholder.
- **UI text scale** in Settings — Compact (0.9×), Default (1.0×),
  Comfortable (1.15×) drive a `--text-scale` CSS variable on `:root`
  so text resizes but button hit-targets and grid spacing stay
  fixed in pixels. Hit-target consistency matters more than uniform
  zoom on a desktop app. Persisted across launches.
- **Roster layout** in Settings — pick Auto (responsive
  `auto-fit, minmax(280px, 320px)` — grows columns as the window
  widens), 2 riders locked, or 3 riders locked. Auto-mode window
  size persists across launches; fixed-mode snaps the window to a
  width that fits the chosen column count.
- **Auto-updater** via `tauri-plugin-updater`. Cold start polls a
  signed `latest.json` on GitHub Releases; if a newer signed .exe
  is available, a thin banner appears at the top of the window
  ("Bifrost v0.X.Y is ready to install — Restart and update").
  Click → background download with progress → signature verified
  against the pubkey baked into the running .exe → passive NSIS
  install → relaunch. The user never re-downloads manually. See
  `docs/RELEASING.md` for the one-time keygen + signed-release
  workflow.
- **Code-split chunks** — `SettingsView` and `RiderAppsRow`
  (companion-site icons + favicon-fetch pipeline) ship as separate
  bundles loaded on demand. The main `index.js` is ~37 KB gzipped;
  users who only use Bifrost to launch the game (no wallet
  workflow) never pay the wallet-integration bytes.
- **Stale-balance indicator** under each rider's stats — "Updated
  5 m ago / stale" computed from a `wallet_balance_fetched_at`
  timestamp written on every successful Sui RPC refresh. Riders
  whose RPC is failing don't silently show stale figures as
  "fresh"; the user can tell at a glance.
- **Missing-sandbox state + one-click remove**. If a user deletes
  a rider's box externally (via Sandboxie's own "Delete Content"
  menu), the rider card flips to a `Missing` state with a warning
  ribbon and a single "Remove rider" button that bypasses the
  archive-first guard. No more orphaned records; no more Sandboxie
  "Invalid box name parameter" popups every reconcile tick.
- **Reconcile tick gates on window visibility** — the 30 s
  background rider-status refresh skips when
  `document.visibilityState === "hidden"` and fires immediately
  on refocus. No wasted Sandboxie shellouts + Sui RPC calls while
  the window is backgrounded.
- **Atomic file persistence** — `riders.json` and `config.json`
  now write through a temp-file-plus-atomic-rename helper so a
  power loss or hard crash mid-write can't leave a zero-byte file
  that erases the user's whole roster on next launch. On parse
  failure at startup, the corrupt file is renamed to
  `<path>.corrupt-<unix>.json` and Bifrost launches with defaults
  rather than refusing to start.
- **Rate-limit backoff** for GitHub Releases lookups: 5 min
  failure-cache TTL on HTTP 403/429 / asset-missing responses so
  re-renders don't burn quota. Sui RPC has the analogous
  per-address backoff for 429/503/JSON-RPC-overload responses.
- **HTTP body size caps** on every download path (favicons 256 KiB,
  EVE Vault zip 50 MiB, Sandboxie installer 50 MiB, Brave portable
  300 MiB) enforced both via Content-Length AND streaming-tally so
  a misbehaving / compromised upstream can't OOM the process.
- **Live upstream-contract tests** for Brave, EVE Vault, and
  Sandboxie. Run on every `cargo test`, hit the real GitHub
  Releases API, skip gracefully on network failure but fail loudly
  if upstream renames an asset or changes their release channel
  mix in a way our matchers don't tolerate. Backed by a daily
  scheduled CI workflow (`upstream-drift.yml`) that runs the same
  tests with a token and opens a tracking issue on failure.
- **Test suite** — 154 Rust tests across unit, contract, and
  upstream-probe layers (was 93 at the start of pre-public
  polish). Includes a cross-boundary drift test that asserts every
  Rust `#[tauri::command]` has a matching TypeScript wrapper in
  `src/lib/tauri.ts`, plus a `RiderStatus` variant-set test that
  pins the enum against the TS string union.

### Changed

- **Project rename**: Bridge → Bifrost. App identifier
  `app.bridge.frontier` → `io.github.valknarr.bifrost` (reverse-DNS
  under the GitHub user namespace, verifiably owned). Internal
  Rust types `BridgeError` / `BridgeConfig` renamed to
  `BifrostError` / `BifrostConfig` to match the product name
  throughout.
- **Build chain upgraded**: Vite 6 → 8 (with explicit `esbuild`
  devDependency for Vite 8's rolldown transition), Svelte plugin
  5 → 7, TypeScript 5 → 6 (tsconfig `baseUrl` removed per TS 6
  deprecation; `paths` migrated to relative form).
- **Sui RPC fetches go concurrent** — each rider's SUI + EVE
  balance call pair runs via `tokio::join!`; riders themselves
  fan out via `FuturesUnordered` with a max-4 concurrency cap.
- **Chromium asset matcher** broadened to accept both
  `brave-v…-win32-x64.zip` and `brave-origin-v…-win32-x64.zip`
  naming forms. The release lookup switched from `/releases/latest`
  to a list-and-scan helper that includes prereleases — Brave tags
  most Windows-bearing builds as prereleases, so the old singleton
  endpoint returned Android-only builds with no Windows ZIP.

### Safety

- Strictly read-only against the EVE Frontier game client and its
  network protocol. Bifrost orchestrates *around* the launcher via
  Sandboxie isolation; it does not inject into, hook, or proxy
  Frontier itself.
- Wallets are never auto-funded or signed-into by Bifrost. The browser
  is launched, the extension is present — the user signs in.
- Sandboxie uninstall is gated by a pre-flight check that refuses to
  proceed if any sandbox has live processes (Inno-Setup can't tear
  down `SbieDrv.sys` while the driver is held open).
- Single process-wide `reqwest::Client` with a recognisable User-Agent;
  30-min in-process cache for GitHub `releases/latest` lookups so the
  Settings panel stays well under the unauthenticated rate limit.

[Unreleased]: https://github.com/valknarr/bifrost/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/valknarr/bifrost/releases/tag/v0.0.2
[0.0.1]: https://github.com/valknarr/bifrost/releases/tag/v0.0.1
