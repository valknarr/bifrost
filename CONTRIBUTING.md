# Contributing to Bifrost

Thanks for considering a contribution. Bifrost is small and intentionally
focused — a clear, friendly multi-rider session manager for EVE Frontier
that respects the game's official APIs. Patches that hold that line are
welcome.

> **Orienting yourself?** [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)
> has the repo layout, design principles, design tokens, and a quick
> tour of state / persistence / network surface. Read that once before
> opening a substantial PR.

## Prerequisites

- **Rust** stable (`rustup default stable`). Toolchain pinned in
  `src-tauri/rust-toolchain.toml`.
- **Node.js** 20+ and **pnpm** 9+ (`npm i -g pnpm`).
- **Microsoft C++ Build Tools** or Visual Studio 2022 with the "Desktop
  development with C++" workload.
- **WebView2 Runtime** (already on Windows 11).

Sandboxie does **not** need to be pre-installed — Bifrost offers to
install it (Plus or Classic) from the Settings panel via the official
installer, with one Windows UAC prompt.

## Quick start — dev mode

```sh
git clone https://github.com/valknarr/bifrost.git
cd bifrost
pnpm install
pnpm tauri dev
```

First `pnpm tauri dev` takes 5–10 minutes to compile the Rust backend on
a cold machine. Subsequent runs are fast.

## Build a release `.exe` locally

```sh
pnpm tauri build
```

Output lands in `src-tauri/target/release/bundle/nsis/`. This is the
same command CI runs from a release tag — but without the signing
secrets, so the resulting `.exe` cannot be used to auto-update
existing installs (the `latest.json` will be missing its signature
and the in-app updater will reject it). For an actual published
release, push a `v*.*.*` tag and let CI build it; see
[`docs/RELEASING.md`](./docs/RELEASING.md).

## What we'd love help with

- **Bug reports** with reproducible steps. Bifrost has a small surface
  but Sandboxie's runtime behaviour can be subtle; concrete traces are
  gold.
- **EVE-Frontier-specific** quality-of-life features: per-rider session
  insights, better wallet/balance presentation, rider grouping, etc.
- **Polishing existing flows** — first-launch hints, error messages
  that tell the user what to do next, accessibility (keyboard
  navigation, screen-reader labels).
- **Cross-version Sandboxie testing** — Bifrost has been built against
  Sandboxie-Plus 1.15+; older versions may have surfaced INI keys
  Bifrost writes but doesn't.

## What we won't merge

- **Anything that bypasses Fenris Creations' TOS.** No DLL injection,
  no input multiplexing, no reverse-engineered protocols. Bifrost
  drives only what Fenris Creations (formerly CCP Games) and
  Sandboxie have publicly documented.
- **Host-browser integrations.** Bifrost ships its own portable Chromium
  for per-rider isolation; touching the user's day-to-day Chrome /
  Edge / Firefox profile is out of scope.
- **Telemetry or analytics.** Bifrost is a local app; it should stay
  local. The only network calls are to GitHub Releases for updates and
  to Sui mainnet RPC for wallet balances.
- **Bundled third-party trademarks or brand assets** (Fenris
  Creations, EVE Frontier, Sandboxie, Brave logos / icons / marks).
  The current EVE-Frontier-adjacent palette is deliberately distinct.

## Development conventions

### Code style

- **Rust**: `cargo fmt` + `cargo clippy --all-targets -- -D warnings`
  + `cargo test --all-targets` must pass. CI enforces all three.
  + Three of the tests are "live upstream contract" probes
    (`live_brave_matcher_finds_a_portable_zip_in_recent_releases`,
    `live_evevault_latest_release_has_expected_asset`,
    `live_sandboxie_latest_release_has_both_variants`). They hit
    the real GitHub Releases API. They skip gracefully on
    network / rate-limit failure but FAIL LOUDLY on a real
    matcher / upstream drift — that's how we catch e.g. Brave
    renaming `brave-v…` → `brave-origin-v…` before users
    report it. Set `BIFROST_SKIP_UPSTREAM_TESTS=1` to skip them
    when working offline. A daily scheduled CI job
    (`.github/workflows/upstream-drift.yml`) runs them with a
    token and opens a tracking issue on failure.
- **TypeScript / Svelte**: `pnpm check` (svelte-check) must pass.
- **Comments**: explain *why*, not *what*. Prefer documenting decisions
  ("we use PowerShell here because tokio's `Command` can't elevate")
  over restating what the code obviously does. Public Rust items
  benefit from a one-line `///` summary even when the function name
  is self-explanatory.

### Commit messages

Conventional-commit-ish prefixes are encouraged but not enforced:

```
feat:     New user-visible capability
fix:      Bug fix
refactor: Code shape change, no behaviour change
chore:    Tooling / housekeeping / dep bumps
docs:     Documentation only
```

Write the subject in the imperative ("add per-rider accent picker"
not "added per-rider accent picker"), keep it under 72 chars, and use
the body to explain the **why** — what problem does this solve, what
alternatives were rejected.

### Pull requests

Open a draft PR early if you're not sure about an approach; we'd
rather catch a wrong turn at design time than after the implementation.
Each PR should:

- Stand on its own (refactor commits separate from behaviour commits
  where practical).
- Pass CI (`cargo fmt --check`, `cargo clippy --all-targets`,
  `pnpm check`).
- Update [`CHANGELOG.md`](./CHANGELOG.md) under the `## [Unreleased]`
  section for any user-visible change.
- Include a screenshot or short clip if the change is visual.

### Versioning + release process

Bifrost follows [Semantic Versioning](https://semver.org/). Pre-1.0 we
treat **any** behaviour change as worth a minor bump. Releases are
tagged `v0.0.X` from `main` and built into a single signed `.exe`
via `pnpm tauri build`, published to GitHub Releases.

## Security disclosures

Please do **not** file public issues for security vulnerabilities. See
[SECURITY.md](./SECURITY.md) for the private reporting channel.

## Code of conduct

Be a decent person. Disagreements happen — assume good faith, debate
the work, not the contributor. Project maintainers will moderate
discussions that drift away from that.

## Questions

Open a [Discussion](https://github.com/valknarr/bifrost/discussions) if
you want to talk through an idea before writing code, or file an issue
with the `question` label. Both are fine.
