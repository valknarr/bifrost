# Bifrost architecture

Reference document — read once for orientation before opening a PR or
diving into the code. The contributor how-to ("set up dev mode", "open
a PR") lives in [CONTRIBUTING.md](../CONTRIBUTING.md); this file is
just the mental model.

## Repo layout

```
.
├── src/                            Svelte 5 frontend
│   ├── App.svelte                  Top-level shell + nav
│   ├── app.css                     Tailwind 4 + design tokens
│   ├── main.ts                     Svelte mount
│   └── lib/
│       ├── tauri.ts                Typed wrappers around invoke()
│       ├── types.ts                Shared TS types (mirror Rust models)
│       ├── external.ts             Shell-plugin `open()` helper + URLs
│       ├── version.ts              `vTag()` helper — strip-leading-v
│       ├── components/             Reusable UI atoms
│       ├── stores/                 Svelte 5 rune-based stores
│       └── views/                  Top-level routes (Riders, Settings)
├── src-tauri/                      Rust backend
│   ├── Cargo.toml
│   ├── rust-toolchain.toml         Pinned compiler
│   ├── tauri.conf.json
│   ├── capabilities/               Permission grants for the frontend
│   ├── audit.toml                  cargo-audit ignore list (GTK3 transitives)
│   └── src/
│       ├── main.rs                 Entry point
│       ├── lib.rs                  Tauri setup + command registration
│       ├── state.rs                Process-wide app state (riders, config, locks)
│       ├── rider.rs                Rider model + lifecycle
│       ├── sandboxie.rs            Wraps SbieIni.exe / Start.exe
│       ├── sandboxie_installer.rs  Silent install/uninstall of Sandboxie (Plus + Classic)
│       ├── browser.rs              Per-rider Brave launcher + theme extension
│       ├── chromium.rs             Portable Brave downloader
│       ├── evevault.rs             EVE Vault extension downloader
│       ├── release_cache.rs        Shared GitHub-fetch helpers + 30-min cache
│       ├── atomic_write.rs         tmp+rename helper for riders.json / config.json
│       ├── config.rs               Persisted settings
│       ├── error.rs                BifrostError + Result alias
│       ├── http.rs                 Process-wide reqwest client + capped downloads
│       ├── ini.rs                  Sandboxie.ini parser
│       ├── sui.rs                  Sui mainnet RPC client + per-address backoff cache
│       └── commands/               Tauri command surface, one module per domain
├── .github/                        CI workflow + issue templates
│   └── workflows/
│       ├── ci.yml                  fmt + clippy + test + audit on every push
│       ├── release.yml             Signed release build on `v*.*.*` tag push
│       └── upstream-drift.yml      Daily probe of Brave / EVE Vault / Sandboxie feeds
├── docs/
│   ├── ARCHITECTURE.md             You are here
│   ├── RELEASING.md                Per-release flow + minisign keygen
│   └── screenshots/                README hero + future screenshots
├── CHANGELOG.md                    Keep-a-Changelog format
├── CONTRIBUTING.md                 How to contribute
├── CODE_OF_CONDUCT.md
├── SECURITY.md                     How to report vulnerabilities
├── LICENSE                         MIT
└── README.md                       User-facing entry point
```

## Design principles

1. **Official APIs only.** No DLL injection, no input multiplexing, no
   reverse-engineered protocols. Bifrost drives only what Fenris
   Creations and the Sandboxie project have publicly documented.
2. **Single portable binary.** One `.exe` from GitHub Releases. The
   only hard dependency is Sandboxie-Plus, which Bifrost installs
   silently on first run.
3. **The user never sees Sandboxie.** Sandboxie is plumbing. Riders,
   sessions, wallets — that's what the UI shows.
4. **Per-rider bundling.** Each rider session is one unit: game client
   + Brave profile + EVE Vault. Switching riders switches identity
   wholesale, not piecemeal.
5. **No telemetry.** Bifrost is a local app. The only network calls are
   to GitHub Releases (for component updates) and the Sui mainnet RPC
   (for wallet balances).

## Design tokens

See `src/app.css`. Bifrost uses an EVE-Frontier-adjacent palette but
deliberately distinct from Fenris Creations' brand colours.

| Token | Value | Use |
|---|---|---|
| `--color-bg` | `#04060a` | App background |
| `--color-surface` | `#0b0e15` | Cards, panels |
| `--color-elevated` | `#141822` | Hover / active surfaces |
| `--color-border` | `#1a1f29` | 1px panel borders |
| `--color-border-hi` | `#262d3a` | Stronger borders |
| `--color-text` | `#eef0f5` | Primary text |
| `--color-text-muted` | `#b2bbd0` | Secondary text |
| `--color-text-dim` | `#7a8398` | Tertiary text |
| `--color-accent` | `#e54b1a` | Primary actions, brand |
| `--color-accent-hi` | `#ff6a30` | Accent hover |
| `--color-focus` | `#f5c542` | Focused window, selected tab |
| `--color-ok` | `#ff6332` | Running riders — brand orange (not green) |
| `--color-warn` | `#f2c94c` | Warnings |
| `--color-danger` | `#e2604a` | Errors, destructive |

Type: JetBrains Mono everywhere (the EVE in-game UI is committed to
monospace). Inter is loaded as a fallback for any non-mono surface.

UI scale is a CSS variable (`--text-scale`), not webview zoom: text
resizes via the variable but button hit-targets and grid spacing stay
in pixels. Hit-target consistency matters more than uniform zoom on
a desktop app.

## State + persistence

Bifrost persists two JSON files under the Tauri-resolved app-data
directory (`%APPDATA%/io.github.valknarr.bifrost/` on Windows):

- **`config.json`** — companion-site list, UI text-scale, roster
  column count, Auto-mode window size.
- **`riders.json`** — the full rider roster (id, display name, sandbox
  name, browser-profile path, wallet address, accent colour, archived
  flag).

Both files go through `atomic_write::write_atomic()` (tmp + rename) so
a crash mid-save can't leave a zero-byte file. On corrupt parse the
file is renamed to `<name>.corrupt-<unix>.json` and the user gets an
empty default state — rebuilding from the still-existing browser
profiles is friendlier than refusing to launch with an opaque error.

Both files are loaded behind `Mutex`es with a poisoning-recovery
helper (`state.riders_lock()` / `state.config_lock()`) so a panic
inside any critical section doesn't cascade to every subsequent
command. Long-running work (Sandboxie CLI shells, GitHub fetches,
browser launches) never holds a lock.

## Network surface

The whole app makes calls to exactly these hosts:

- `api.github.com` + `github.com` — release lookups for Sandboxie,
  Brave, EVE Vault; auto-updater manifest. Cached 30 min in-process
  to stay well under the unauth 60 req/hr GitHub rate limit, with a
  5-minute rate-limit backoff on `403` / `429`.
- `fullnode.mainnet.sui.io` — Sui RPC for wallet balances. Per-address
  backoff cache (2 min) on rate-limit / RPC errors.
- Per-companion-site origins — favicon fetched once at config-add
  time, cached in `<app-data>/favicons/`. Not re-fetched at runtime.

All HTTP goes through a single process-wide `reqwest::Client` with a
recognisable User-Agent. Download paths use `http::download_capped()`
so a misbehaving upstream can't blow the heap by streaming an
unbounded body — caps: favicons 256 KiB, EVE Vault 50 MiB, portable
Brave 300 MiB, Sandboxie installer 50 MiB.

## Update flow

Tauri's `tauri-plugin-updater` polls a signed `latest.json` manifest
on cold start. If the manifest's version is newer than the running
binary AND the signature verifies against the pubkey baked in at
build time, a banner appears at the top of the window. Click →
download the new `.exe` → verify signature → passive NSIS install →
relaunch. See [RELEASING.md](./RELEASING.md) for the keygen + signed-
release workflow that produces the manifest.
