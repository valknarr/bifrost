# Changelog

All notable changes to Bifrost will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Nothing yet._

## [0.0.5] - 2026-05-25

### Added

- **Frontend test foundation — Vitest + jsdom.** Closes the pre-1.0
  TODO entry. New `vitest.config.ts` + `src/test-setup.ts` that
  stubs `@tauri-apps/api/core::invoke` and `@tauri-apps/api/app::
  getVersion` so tests don't need a real Tauri runtime. `pnpm test`
  added as a script + a CI step in `.github/workflows/ci.yml`
  (runs after `svelte-check`, under 2 s). Two starter test files
  shipping with this release: `src/lib/version.test.ts` (11
  assertions pinning `vTag()` against the `vv0.0.3` display bug
  class) and `src/lib/error.test.ts` (5 assertions pinning
  `formatBackendError` against the prefix-strip contract). Store
  and component tests are deferred to v0.0.6 — wiring the
  framework + the smallest pure-function tests is the durable
  foundation; bigger tests slot on top without further scaffolding.

### Fixed

- **`delete_rider`: save-before-mutate ordering.** Previously the
  in-memory rider Vec was mutated first, then `save_riders()`
  wrote the new roster to disk. If the disk write failed (full
  disk, antivirus blocking the atomic-rename, etc.) the rider was
  gone from memory but still in `riders.json` on disk — next
  launch resurrected a "rider" whose Sandboxie box + per-rider
  filesystem state had already been torn down by the rest of the
  command. Reordered: snapshot under the lock, persist the new
  list to disk via the new `AppState::replace_riders_and_save`
  helper, commit the in-memory swap ONLY on successful write.
  Memory and disk now stay consistent across the save boundary.
- **Atomic version-marker writes** in `chromium`, `evevault`, and
  `sandboxie_installer`. The `.bifrost-version` /
  `.bifrost-installed-version` markers were written via
  `std::fs::write` (truncate + write); a power loss between the
  two steps would leave a zero-byte marker that
  `read_installed_version` then returned as `None`, with the
  Settings UI reporting "external install / unknown version"
  forever — no UI path back to truth. All three sites now route
  through `atomic_write::write_atomic` (tmp + rename).
- **Install staging — extract to `.new`, atomic swap, remove
  `.old`** for both `chromium::install` and `evevault::install`.
  Previous shape was `remove_dir_all(target)` + extract in place;
  a mid-extract failure (zip-bomb cap, IO error, AV interference)
  left the user with no install at all when they had a working one
  before. The new shape: extract into `target.new`, then on full
  success `rename(target → target.old); rename(target.new →
  target); remove_dir_all(target.old)`. A failed rename triggers
  rollback (`rename(target.old → target)`) so the user is never
  left in a no-install state.
- **`start_rider`: Drop-revert guard.** A failed launch
  (`provision_frontier_box` errors, `launch_in_box` errors) left
  the rider wedged in `Starting` until the next
  `reconcile_riders` tick (≥30 s later) — the user saw the error
  toast but the card kept spinning. A small `Drop`-impl guard on
  the start path now flips the rider back to `Stopped`
  synchronously on any error before commit, so the card snaps
  back the moment the toast appears.
- **Blocking `std::fs` calls in async hot paths**: `delete_rider`'s
  retry loop and `sandboxie::delete_box`'s box-data-wipe both
  called `std::fs::remove_dir_all` inside async functions. With a
  large per-rider tree (~hundreds of MB: Brave profile + EVE Vault
  state + game cache), each call could stall a Tokio worker for
  hundreds of ms. Converted both to `tokio::fs::remove_dir_all`
  so the runtime stays responsive to in-flight balance refreshes,
  status probes, and reconcile ticks.

### Changed

- **Maintainer identity in `Cargo.toml`.** Was `"Bifrost
  contributors"`; now `"valknarr <valknarr@pm.me>"` — matching the
  contact address already in `CODE_OF_CONDUCT.md` and
  `SECURITY.md`. Real-name maintainer signal is part of the v0.1.0
  readiness work flagged in the pre-1.0 audit.

## [0.0.4] - 2026-05-25

### Fixed

- **CRITICAL: Brave installer was shipping the wrong product.** The
  v0.0.2 / v0.0.3 asset matcher accepted both `brave-v…-win32-x64.zip`
  (regular **Brave Browser**) and `brave-origin-v…-win32-x64.zip`
  (which is **Brave Origin**, a separate paid Brave variant that
  prompts for a license purchase on first launch). The original
  matcher comment claimed `brave-origin-v…` was just an internal
  build-chain prefix — it isn't, it's a different product. Users
  who clicked "Update Brave" in Settings during v0.0.3 may have
  received Brave Origin instead of Brave Browser; the wrong app
  shows a "Welcome to Brave Origin — purchase or restore" screen.

  **Cleanup recipe** if you're affected:
  Settings → Portable Browser → **Uninstall** → **Install** again.
  The fixed matcher will skip Brave-Origin-only releases and land
  on the most recent regular-Brave release (currently v1.92.92).
  Per-rider browser profiles under `<app-data>/riders/*/browser/`
  are preserved across the reinstall.
- **`Layout.svelte` grid template was missing the row for the
  update banner.** Three rows declared (`48px 1fr 28px`) but four
  grid children rendered when the auto-updater had a pending
  release. CSS auto-placement collapsed the main content area to
  28 px and the rider roster vanished while an update was pending.
  Added the explicit `auto` row for the banner so it collapses to
  0 px when no update is available and sizes naturally when one is.
- **Stale error retention** in `chromiumStore`, `vaultStore`, and
  `sandboxieStore` — `refresh()` didn't clear `this.error` on
  entry, so a stale install-failure error stayed pinned in red
  even after a subsequent successful status refresh.
  `statusStore` did this correctly; the other three were the
  regression.
- **Brave install/uninstall pre-flight now distinguishes Bifrost's
  portable Brave from the user's regular Brave install.** The
  v0.0.3 check (added for the Brave-running uninstall error) used
  `tasklist /FI "IMAGENAME eq brave.exe"` which counted EVERY
  `brave.exe` on the host — including a user's daily-driver Brave
  in `Program Files`. Result: a user with their normal Brave open
  browsing the web couldn't install or update Bifrost's portable
  Brave even though the two installs don't share any files.
  Rewritten to use `Get-CimInstance Win32_Process` with the
  `ExecutablePath` column, filtering to processes whose .exe lives
  inside Bifrost's chromium install directory. Same shape as the
  `kill_browsers_for_profile` pattern in `browser.rs` (which
  filters by `--user-data-dir`).
- **Pre-flight extended to `install_chromium`, not just
  `uninstall_chromium`.** `install` does `remove_dir_all` +
  extract over the same path; if a Bifrost-managed Brave was
  running during that flow (e.g. a per-rider window from clicking
  Apps a moment earlier, or a stale Brave Origin window from
  v0.0.3) the wipe was partial and the extract overlaid new files
  onto locked old ones. The resulting hybrid install launched
  with `ERROR:scoped_file_writer.cc:17] Could not open pak file
  for writing` and other Chromium sharing-violation noise.
  Refusing the install with a clear "close every rider's browser
  window first" message mirrors the same guard on `uninstall`.

### Added

- **Release cache is now disk-backed across restarts.** Bifrost's
  GitHub Releases lookups (Sandboxie, Brave, EVE Vault, plus the
  updater check) all flow through `release_cache::fetch_with_cache`
  with a 30-min success TTL and a 5-min rate-limit-failure backoff
  TTL. Until v0.0.4 both lived in an in-process `HashMap` that was
  wiped on every app restart — so a development restart-flurry, or
  a real user who closes-and-reopens the app within the same
  half-hour, kept re-firing the same lookups and could exhaust
  GitHub's 60 req/hr unauthenticated quota.

  Cache now persists to `<app-data>/release-cache.json` (atomically
  via the existing `atomic_write::write_atomic`) on every successful
  fetch AND every rate-limit-failure record. On app boot,
  `release_cache::init_disk_cache` (wired from `AppState::new`)
  loads the file and seeds the in-memory cache with non-expired
  entries; expired entries are dropped. Net effect for the user:
  closing-and-reopening Bifrost within the 30-min TTL hits 0
  GitHub calls instead of 4, and a rate-limit hit from a previous
  session is correctly remembered for the full 5 min instead of
  re-firing immediately. ETag/304-conditional support and an
  optional `GITHUB_TOKEN` env var are planned for v0.0.5 / v0.1.0.

### Changed

- **Trademark + version disclaimers.** Several documentation
  references no longer matched the code:
  - `README.md` "Known limitations" referenced a non-existent
    `Sandboxie::version()` method; rewritten to point at the
    actual `read_installed_marker` path.
  - `README.md` install section claimed the `.sig` "beside the
    `.exe`" lets users verify integrity; clarified that the
    embedded minisign signature in `latest.json` is what the
    auto-updater uses, with `SHA256SUMS.txt` (pre-1.0 TODO)
    being the eventual manual-verification path.
  - `CHANGELOG.md` v0.0.1 entry claimed Sui RPC fetches use
    `FuturesUnordered` with a max-4 concurrency cap. The actual
    code is sequential per-rider (Sui's public mainnet RPC
    throttles per-IP on bursts). Annotation updated to match.
  - `src-tauri/src/favicon.rs` module-level comment said the
    User-Agent was `bifrost/0.0.1`; it's actually built from
    `CARGO_PKG_VERSION` at compile time. Comment now reflects.
  - `src-tauri/src/config.rs` test comment said "Pre-v0.0.4"
    when it should have said "Pre-v0.0.1" — the field has
    existed since the first release.
- **`tauri.ts` naming consistency.** `installEveVault` / camelcase
  `EveVault` already, but `uninstallEvevault` used lowercase `v`.
  Renamed to `uninstallEveVault` in the TS wrapper. No backend
  rename (Rust command name `uninstall_evevault` stays — the
  cross-boundary drift test pins the Rust name, the TS rename is
  cosmetic on the wrapper side).
- **JSDoc on `useClockTick`** was actively misleading — it told
  callers to `onMount(() => useClockTick())`, but the helper
  already calls `onMount` internally. Following the doc silently
  broke cleanup. Doc rewritten to say "call at the top of
  `<script>` — wires its own `onMount` and cleanup."

## [0.0.3] - 2026-05-24

### Added

- **App version in the footer** — bottom-left of the window now reads
  `BIFROST v<version>` instead of repeating the active tab title.
  Pulled from `tauri.conf.json` via `getVersion()` at app boot.
- **About section in Settings** — version, manual "Check for updates"
  button (force-polls the GitHub Releases endpoint even in dev
  builds), and external links to the repo / release notes /
  changelog. The manual check surfaces "● You're on the latest
  release" briefly on a no-op poll and reuses the existing top-of-
  window banner when an update is found.

### Changed

- **Balance-freshness indicator is now error-only.** Originally a
  three-tier "Updated just now / 2m ago / stale" stamp on every
  Rider card, the freshness label redrew every 15 s and caused
  minor layout jitter as the text width changed. It also didn't
  tell the user anything actionable while everything was working.
  Now the row is hidden entirely while the balance is fresh
  (refreshed within the last 5 min) and surfaces only when the
  Sui RPC has been failing for at least 5 minutes, with a
  warn-yellow "⚠ Balance stale · Xm old" label. Same staleness
  threshold as before, much quieter UI.

### Fixed

- **Brave-uninstall pre-flight** — uninstalling the portable browser
  while any `brave.exe` was running previously failed deep inside
  `remove_dir_all` with `os error 5 (Access denied)`, leaving the
  install half-deleted. Bifrost now counts running Brave processes
  via `tasklist /FI "IMAGENAME eq brave.exe"` before touching the
  filesystem and refuses with a clear "close every Brave window
  first" message if any are alive. Mirrors the
  "refuse uninstall while sandboxes are active" guard we already
  use for Sandboxie.

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
- **Sui RPC fetches go concurrent _within_ a rider** — each rider's
  SUI + EVE balance call pair runs via `tokio::join!`. Riders
  themselves are iterated sequentially because Sui's public mainnet
  RPC throttles per-IP on bursts (see the comment in
  `commands/wallet.rs::refresh_balances`).
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

[Unreleased]: https://github.com/valknarr/bifrost/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/valknarr/bifrost/releases/tag/v0.0.5
[0.0.4]: https://github.com/valknarr/bifrost/releases/tag/v0.0.4
[0.0.3]: https://github.com/valknarr/bifrost/releases/tag/v0.0.3
[0.0.2]: https://github.com/valknarr/bifrost/releases/tag/v0.0.2
[0.0.1]: https://github.com/valknarr/bifrost/releases/tag/v0.0.1
