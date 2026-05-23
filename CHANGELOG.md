# Changelog

All notable changes to Bifrost will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Pilot roster** — designate, launch, archive, restore, and delete EVE
  Frontier pilots. Each pilot is backed by a Sandboxie box and a Brave
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
    into every per-pilot browser session.
- **Per-pilot browser launch** — each pilot opens its own Brave window
  inside its sandbox, with the EVE Vault extension preloaded and a
  small generated theme extension that tints the window frame with the
  pilot's accent colour. Multiple pilots can have their wallet sessions
  open simultaneously without overlapping.
- **Companion sites** — built-in EVE Map link and a custom-site
  manager. Each pilot's card has an Apps row that opens these sites
  using the pilot's wallet identity. Built-ins can be hidden but not
  removed; custom sites can be added and removed at will.
- **Sui mainnet wallet readout** — Bifrost reads SUI + EVE token
  balances for each pilot's wallet via the public `suix_getBalance`
  JSON-RPC endpoint. Read-only — Bifrost never signs or submits a
  transaction.
- **Setup banner + dual-state HUD** — the Pilots view warns when host
  dependencies are missing (Sandboxie not installed, EVE Frontier
  client not found) with one-click links into Settings. The top-right
  header shows a two-dot indicator: pilot lifecycle (Online · N/N)
  plus system health (System / Setup).
- **EVE-Frontier-aligned visual language** — sharp rectangles, mono
  type, red-orange accent, `[#]`-bracketed window titles. Sparse
  procedural multi-galaxy background renders three slowly-spinning
  spiral galaxies that drift toward each other over many sessions
  (floored at 50 % of initial separation so they never actually
  merge). Pilot cards carry corner brackets, an "energy" bar in the
  accent colour, and a halftone portrait placeholder.
- **UI zoom presets** in Settings — Compact (0.9×), Default (1.0×),
  Comfortable (1.15×) call Tauri's `webview.setZoom()` so every pixel
  scales together. Persisted across launches.
- **Cascading integration test suite** (93 tests). Real on-disk
  sequences exercise the pilot model, config round-trips, version
  marker contracts, release-cache TTL behaviour, EVE Vault manifest
  parsing, Sandboxie installer asset matching (Plus + Classic), and
  the CLI helper formatters.

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

[Unreleased]: https://github.com/valknarr/bifrost/commits/main
