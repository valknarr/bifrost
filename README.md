# Bridge

**An open-source multi-pilot session manager for EVE Frontier.**

One click → N isolated pilot sessions, each with its own game client,
browser profile, and EVE Vault wallet. No keystroke broadcasting, no DLL
injection, no TOS edge cases. Bridge wraps CCP's officially recommended
sandboxing tool ([Sandboxie-Plus]) behind a calm UI, and bundles per-pilot
[Brave Browser] + [EVE Vault] so each pilot has a fully isolated identity
out of the box.

> Bridge is an unofficial community tool. It is not affiliated with or
> endorsed by CCP Games. "EVE Frontier" is a trademark of CCP hf.

## Why

Multiboxing in EVE Frontier means juggling N game clients, N wallet
sessions, and a wallet extension that only loads in one browser profile
at a time. The community has solved this with hand-rolled `.bat` scripts
that hand-craft Sandboxie configs, but the user experience is rough and
easy to get wrong (orphaned sandboxes, mixed wallet sessions, "which
pilot is that?").

Bridge replaces all of that with:

- **One UI** that shows pilots, not sandboxes.
- **One installer** that brings its own portable Brave + EVE Vault, so
  the user's day-to-day browser is never touched.
- **One sign-in per pilot, ever.** Wallet sessions persist between
  launches because each pilot owns a real Chromium profile.
- **Coloured window frames per pilot** — Airikr's wallet windows are
  orange, Tal'Ra's green, etc. — so you always know which identity
  you're acting as.

## Status

Pre-alpha. Active development. The Pilots view, Sandboxie + EVE Vault
integration, per-pilot browser sessions, and wallet balance reads from
the Sui mainnet RPC all work today. See [`CHANGELOG.md`](./CHANGELOG.md)
for what landed when.

## Prerequisites

- **Rust** stable (`rustup default stable`). Toolchain pinned in
  `src-tauri/rust-toolchain.toml`.
- **Node.js** 20+ and **pnpm** 9+ (`npm i -g pnpm`).
- **Microsoft C++ Build Tools** or Visual Studio 2022 with the "Desktop
  development with C++" workload.
- **WebView2 Runtime** (already on Windows 11).

Sandboxie does **not** need to be pre-installed — Bridge offers to
install it (Plus or Classic) from the Settings panel via the official
installer, with one Windows UAC prompt.

## Quick start

```sh
git clone https://github.com/valknarr/Bridge.git
cd Bridge
pnpm install
pnpm tauri dev
```

The first `pnpm tauri dev` will compile the Rust backend (5–10 min on a
cold machine, fast after that).

## Build a release `.exe`

```sh
pnpm tauri build
```

Output lands in `src-tauri/target/release/bundle/`.

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
│       ├── components/             Reusable UI atoms
│       ├── stores/                 Svelte 5 rune-based stores
│       └── views/                  Top-level routes
├── src-tauri/                      Rust backend
│   ├── Cargo.toml
│   ├── rust-toolchain.toml         Pinned compiler
│   ├── tauri.conf.json
│   ├── capabilities/               Permission grants for the frontend
│   └── src/
│       ├── main.rs                 Entry point
│       ├── lib.rs                  Tauri setup + command registration
│       ├── pilot.rs                Pilot model + lifecycle
│       ├── sandboxie.rs            Wraps SbieIni.exe / Start.exe
│       ├── sandboxie_installer.rs  Silent install/uninstall of Sandboxie (Plus + Classic)
│       ├── browser.rs              Per-pilot Brave launcher + theme extension
│       ├── chromium.rs             Portable Brave downloader
│       ├── evevault.rs             EVE Vault extension downloader
│       ├── release_cache.rs        Shared GitHub-fetch helpers + 30-min cache
│       ├── config.rs               Persisted settings
│       ├── error.rs                BridgeError + Result alias
│       ├── ini.rs                  Sandboxie.ini parser
│       └── sui.rs                  Sui mainnet RPC client
├── .github/                        CI workflow + issue templates
├── CHANGELOG.md                    Keep-a-Changelog format
├── CONTRIBUTING.md                 How to contribute
├── CODE_OF_CONDUCT.md
├── SECURITY.md                     How to report vulnerabilities
├── LICENSE                         MIT
├── package.json                    Frontend deps
├── pnpm-lock.yaml
├── svelte.config.js
├── tsconfig.json
├── vite.config.ts
└── README.md                       You are here
```

## Design principles

1. **Official APIs only.** No DLL injection, no input multiplexing, no
   reverse-engineered protocols. Bridge drives only what CCP and the
   Sandboxie project have publicly documented.
2. **Single portable binary.** One `.exe` from GitHub Releases. The
   only hard dependency is Sandboxie-Plus, which Bridge installs
   silently on first run.
3. **The user never sees Sandboxie.** Sandboxie is plumbing. Pilots,
   sessions, wallets — that's what the UI shows.
4. **Per-pilot bundling.** Each pilot session is one unit: game client
   + Brave profile + EVE Vault. Switching pilots switches identity
   wholesale, not piecemeal.
5. **No telemetry.** Bridge is a local app. The only network calls are
   to GitHub Releases (for component updates) and the Sui mainnet RPC
   (for wallet balances).

## Design tokens

See `src/app.css`. Bridge uses an EVE-Frontier-adjacent palette but
deliberately distinct from CCP's brand colours.

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
| `--color-ok` | `#ff6332` | Running pilots — brand orange (not green) |
| `--color-warn` | `#f2c94c` | Warnings |
| `--color-danger` | `#e2604a` | Errors, destructive |

Type: JetBrains Mono everywhere (the EVE in-game UI is committed to
monospace). Inter is loaded as a fallback for any non-mono surface.

## Linting

Bridge holds a zero-warning baseline. CI enforces all of these on
every push:

```sh
# Backend (from src-tauri/)
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# Frontend (from repo root)
pnpm check
```

## Acknowledgements

Bridge stands on the shoulders of several open-source projects:

- **[Sandboxie-Plus][Sandboxie-Plus]** — the kernel-level sandboxing
  engine that makes per-pilot isolation possible. Bridge calls
  Sandboxie's CLI tools (`SbieIni.exe`, `Start.exe`) and ships the
  official silent installer; we don't link `SbieDll.dll`.
- **[Brave Browser][Brave Browser]** — the Chromium fork Bridge bundles
  as its portable per-pilot browser. We picked Brave specifically
  because it ships with the full Google-identity plumbing that
  FusionAuth's OAuth flow (used by EVE Vault) needs.
- **[EVE Vault][EVE Vault]** — the official Chromium wallet extension
  Bridge side-loads into each pilot's profile.
- **[Tauri](https://tauri.app/)** — the desktop runtime.
- **[Svelte](https://svelte.dev/)** — the frontend framework.

CCP Games' EVE Frontier visual language inspired the UI palette and
typography without using any CCP brand assets directly.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for prerequisites, dev-mode
instructions, code style, and the bar for accepted patches.

## Security

Please do not report security vulnerabilities through public GitHub
issues. See [SECURITY.md](./SECURITY.md) for the private channel.

## Licence

MIT — see [LICENSE](./LICENSE).

[Sandboxie-Plus]: https://github.com/sandboxie-plus/Sandboxie
[Brave Browser]: https://github.com/brave/brave-browser
[EVE Vault]: https://github.com/evefrontier/evevault
