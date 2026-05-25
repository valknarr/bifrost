# Bifrost

[![CI](https://github.com/valknarr/bifrost/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/valknarr/bifrost/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Latest release](https://img.shields.io/github/v/release/valknarr/bifrost?include_prereleases&label=latest&sort=semver)](https://github.com/valknarr/bifrost/releases/latest)

**Multi-rider session manager for EVE Frontier.**

![Rider roster — three riders, one launched, each with its own
coloured frame and wallet session.](./docs/screenshots/riders.png)

One click → N isolated rider sessions, each with its own game client,
browser profile, and EVE Vault wallet. No keystroke broadcasting, no
DLL injection, no TOS edge cases.

> Bifrost is an unofficial community tool. Not affiliated with or
> endorsed by Fenris Creations (formerly CCP Games). "EVE Frontier"
> is a trademark of CCP ehf., doing business as Fenris Creations.

## Install

1. Download **`Bifrost_<version>_x64-setup.exe`** from the
   [**latest release**](https://github.com/valknarr/bifrost/releases/latest).
2. Run it. Windows SmartScreen will warn that the installer is
   from an unrecognised publisher — click **More info → Run anyway**.
   (Authenticode signing is a [pre-1.0 TODO](#roadmap--pre-10-todo).
   Every release ships a `latest.json` manifest with an embedded
   minisign signature; the auto-updater verifies it against the
   pubkey baked into your running binary before installing any
   update. For first-install manual verification, see
   [SECURITY.md](./SECURITY.md) — a `SHA256SUMS.txt` step is also
   on the pre-1.0 roadmap.)
3. On first launch, Bifrost offers to install Sandboxie (Plus or
   Classic) silently. One Windows UAC prompt; that's the only
   external dependency.

That's it. Future updates land via an in-app banner — Bifrost polls
GitHub Releases on cold start, verifies the next release's signature
against the pubkey baked into the running `.exe`, and offers a
one-click upgrade.

**Want to build from source or contribute?** See
[CONTRIBUTING.md](./CONTRIBUTING.md) for dev setup and
[`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for repo layout and
design notes.

## Why

Multiboxing in EVE Frontier means juggling N game clients, N wallet
sessions, and a wallet extension that only loads in one browser profile
at a time. The community has solved this with hand-rolled `.bat` scripts
that hand-craft Sandboxie configs, but the user experience is rough and
easy to get wrong (orphaned sandboxes, mixed wallet sessions, "which
rider is that?").

Bifrost replaces all of that with:

- **One UI** that shows riders, not sandboxes.
- **One installer** that brings its own portable Brave + EVE Vault, so
  the user's day-to-day browser is never touched.
- **One sign-in per rider, ever.** Wallet sessions persist between
  launches because each rider owns a real Chromium profile.
- **Coloured window frames per rider** — Airikr's wallet windows are
  orange, Tal'Ra's green, etc. — so you always know which identity
  you're acting as.

## Status

**v0.0.x — actively developed.** Daily-driver stable for the maintainer
on Windows 11; binaries auto-update via [Tauri's signed
updater](docs/RELEASING.md) so once you're installed you stay on the
latest. Recommended for users comfortable with `.exe` installs from
GitHub Releases; a one-click installer flow for non-technical users
lands in v0.1.

The Riders view, Sandboxie + EVE Vault integration, per-rider browser
sessions, wallet balance reads from the Sui mainnet RPC, and the
in-app installers (Sandboxie Plus or Classic, portable Brave, EVE
Vault extension) all work today. See [`CHANGELOG.md`](./CHANGELOG.md)
for what landed when.

The API surface for advanced contributors (Tauri commands, store
shapes, file formats on disk) may still shift between point
releases; no breaking changes to user-visible state will land
without a major bump.

## Known limitations

Things that work today but have a sharp edge worth knowing about.
Listed for transparency rather than tracked for fix unless someone
hits one in practice.

- **EVE Vault download verification is best-effort.** Bifrost fetches
  the official extension from `github.com/evefrontier/evevault` and
  verifies its SHA-256 against the upstream `checksums.txt` when
  that sidecar is present. If a future EVE Vault release ships
  without `checksums.txt`, Bifrost logs a warning and installs the
  zip anyway — but doesn't yet surface "unverified" in the UI.
  See `src-tauri/src/evevault.rs::install`. Mitigation: GitHub
  serves the release artifact over TLS; the substitution surface
  is essentially "GitHub itself."
- **`delete_rider` is not atomic across the save + filesystem-wipe
  boundary.** Bifrost removes the rider from `riders.json` and saves
  the config *before* wiping the per-rider directory under
  `<app-data>/riders/<id>/`. A crash in that ~1 second window
  leaves an orphaned ~200 MB browser profile the UI can't see
  anymore. No data loss — just disk slowly leaks until you nuke
  `<app-data>` manually. Reproducing requires a power-cycle at
  exactly the wrong moment.
- **Sandboxie version detection trusts Bifrost's marker over the
  actual installed binary.** `read_installed_marker` in
  `sandboxie_installer.rs` returns the variant + tag Bifrost wrote
  the last time it ran the installer, not what's currently on disk.
  If the user updates Sandboxie via its own auto-updater rather
  than through Bifrost's Settings panel, the version line in the
  Detection row may lag until they trigger an update through
  Bifrost itself.
- **`delete_box` assumes the default Sandboxie data root
  (`C:\Sandbox\<user>\<box>\`).** If you've customised
  `FileRootPath` in Sandboxie's own settings, deleting a box via
  Bifrost will correctly remove the config section but leave the
  data directory behind under your custom path. The box still
  works as removed (no UI / kernel impact); only the on-disk
  cleanup is incomplete.

## Roadmap / Pre-1.0 TODO

Tracked here rather than as Issues so contributors can see at a glance
what's still rough. Items move to GitHub Issues once someone (or
Dependabot) starts on them.

**Repo hygiene**

- [x] **Explicit Content Security Policy in `tauri.conf.json`** —
      landed in v0.0.2. `default-src 'self'; script-src 'self';
      style-src 'self' 'unsafe-inline'; img-src 'self' data:;
      connect-src 'self' ipc: https://ipc.localhost; …`. Closes the
      supply-chain attack class where a compromised dep silently
      exfiltrates Sui wallet addresses from the webview. Full policy
      + threat model in [SECURITY.md](./SECURITY.md).
- [x] **Branch protection on `main`** — `protect-main` ruleset
      enforces the `Rust (fmt + clippy + test)` + `Frontend
      (svelte-check)` status checks, requires up-to-date branches
      before merging, blocks force-push and deletion. Repo admins
      can bypass for emergency self-rescue.

**Release pipeline**

- [ ] **Authenticode-sign the NSIS installer** (Bifrost installs a
      kernel driver — an unsigned installer + UAC is a poor first
      impression). Gate on a `WINDOWS_CERT_PFX` repo secret so the
      step no-ops until a code-signing cert is available. Until
      then, document SHA-256 verification in `SECURITY.md`.
- [ ] **Publish `SHA256SUMS.txt`** alongside the `.exe` in
      `release.yml` so users can verify the download integrity.
- [x] **`cargo audit` and `pnpm audit` hard-fail CI on any new
      advisory.** Baseline triaged clean as of the pre-public sweep;
      see `src-tauri/audit.toml` for the small ignore-list (GTK3
      transitives, Linux-only, dead code on Windows).

**Test coverage**

- [ ] **Frontend tests** — currently zero. Wire up `vitest` and
      plant at least one spec for `src/lib/error.ts` (round-trip),
      one for a rider-store mutation, and a smoke test for
      `RiderCard.svelte`. Adds a meaningful signal to Dependabot
      bumps.

**Tooling**

- [ ] **Add a frontend lint pass** (eslint or biome) to CI so the
      Rust-side `clippy -D warnings` rigor extends to TS/Svelte.
- [ ] **Pin GitHub Actions by SHA**, not by major tag, in
      `release.yml`. `tauri-action` is already SHA-pinned; the
      other five (`actions/checkout`, `actions/setup-node`,
      `pnpm/action-setup`, `dtolnay/rust-toolchain`,
      `Swatinem/rust-cache`) still ride major-tag refs.

**Wallet UX**

- [ ] Rework the EVE Vault first-launch flow. Right now opening any
      Apps button triggers the OAuth setup as a side effect —
      functional but unexplained. A dedicated wallet-setup state on
      the rider card would make it discoverable.

## Acknowledgements

Bifrost stands on the shoulders of several open-source projects:

- **[Sandboxie][Sandboxie]** — the kernel-level sandboxing engine that
  makes per-rider isolation possible. Bifrost supports both the modern
  Plus build (default) and the Classic LTS build, calling Sandboxie's
  CLI tools (`SbieIni.exe`, `Start.exe`) and shipping the official
  silent installer; we don't link `SbieDll.dll`.
- **[Brave Browser][Brave Browser]** — the Chromium fork Bifrost bundles
  as its portable per-rider browser. We picked Brave specifically
  because it ships with the full Google-identity plumbing that
  FusionAuth's OAuth flow (used by EVE Vault) needs.
- **[EVE Vault][EVE Vault]** — the official Chromium wallet extension
  Bifrost side-loads into each rider's profile.
- **[Tauri](https://tauri.app/)** — the desktop runtime.
- **[Svelte](https://svelte.dev/)** — the frontend framework.

Fenris Creations' EVE Frontier visual language inspired the UI
palette and typography without using any Fenris Creations brand
assets directly.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for prerequisites, dev-mode
instructions, code style, and the bar for accepted patches.
[`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) covers the repo
layout, design principles, and design tokens — read that first if
you're orienting yourself before opening a PR.

## Security

Please do not report security vulnerabilities through public GitHub
issues. See [SECURITY.md](./SECURITY.md) for the private channel.

## Licence

MIT — see [LICENSE](./LICENSE).

[Sandboxie]: https://github.com/sandboxie-plus/Sandboxie
[Brave Browser]: https://github.com/brave/brave-browser
[EVE Vault]: https://github.com/evefrontier/evevault
