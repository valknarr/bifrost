# Threat model — Bifrost

This document is for security-curious users and for reviewers who
want to evaluate Bifrost without reverse-engineering it from the
source tree. It's a snapshot, not a contract: anything below can
shift between releases (the CHANGELOG and `SECURITY.md` track
material changes).

## What Bifrost is

A Windows-only desktop session manager for **EVE Frontier**
multi-Rider play. Bifrost orchestrates around the official EVE
Frontier launcher using publicly-documented APIs — it does NOT
inject, hook, or proxy the game itself. Per-Rider isolation is
provided by [Sandboxie] (kernel-level boxes), and each Rider gets
its own [Brave] browser profile with the [EVE Vault] wallet
extension preloaded.

## Assets — what we care about protecting

In rough order of impact:

1. **The user's Sui wallet identity per Rider.** Bifrost never
   touches Sui private keys — the EVE Vault extension owns them
   inside the browser profile. What Bifrost DOES handle: each
   Rider's **public** Sui address (stored in `riders.json`) and
   periodic read-only balance snapshots from the Sui mainnet RPC.
2. **The pubkey baked into each released `.exe`.** This is the
   minisign verification anchor for the auto-updater. If the
   matching private key is compromised, an attacker can publish a
   "v1.0" update with valid signature and every installed Bifrost
   will trust it.
3. **`riders.json` + `config.json`** on disk. Contains rider names,
   per-rider sandbox names, custom companion sites. No secrets, but
   tampering would mis-target launch commands.
4. **Per-Rider Brave profile dirs** (`<app-data>/riders/*/browser/`).
   Contains the EVE Vault extension state — including the user's
   wallet session cookie after first login. Treat the same as you
   would treat any browser profile on the machine.

Not assets (out of scope):
- The game client itself — Bifrost runs the official launcher in a
  sandbox, but Sandboxie's escape model is Sandboxie's problem,
  not Bifrost's.
- Sui blockchain integrity — public chain, addresses are public,
  the wallet extension lives in its own browser profile.

## Trust boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│  Bifrost.exe (Rust + Svelte WebView)                            │
│  ┌──────────────────────────┐    ┌─────────────────────────┐    │
│  │ WebView (CSP-locked)     │◄───┤ Rust backend            │    │
│  │ - Svelte UI              │    │ - reqwest::Client       │    │
│  │ - cannot reach internet  │    │ - tasklist / PowerShell │    │
│  │ - IPC only via tauri://  │    │ - SbieIni.exe / Start.exe│    │
│  └──────────────────────────┘    └────────┬────────────────┘    │
└────────────────────────────────────────────│────────────────────┘
                                             │
                          ┌──────────────────┼──────────────────┐
                          ▼                  ▼                  ▼
                   GitHub Releases   Sui mainnet RPC     Per-Rider Brave
                   (signed binary +  (read-only)          (sandboxed,
                    .json manifest)                        --user-data-dir)
```

- **WebView ↔ backend**: typed Tauri commands via `src/lib/tauri.ts`.
  No filesystem capabilities; no shell-execute; no asset-protocol
  bypass.
- **Backend ↔ network**: every outbound HTTP goes through one
  process-wide `reqwest::Client` with bounded body caps + rate-limit
  backoff caches. Hosts: `api.github.com`, `github.com`,
  `fullnode.mainnet.sui.io`, per-companion-site favicon origins
  (once at config-add time).
- **Backend ↔ Sandboxie**: shell-out to `SbieIni.exe` /
  `Start.exe` with `cmd.args(...)` (not string interpolation —
  no shell expansion).
- **Per-Rider browsers ↔ network**: out of Bifrost's hands. Each
  Brave instance runs inside a Sandboxie box and talks to whatever
  the user navigates to. The wallet flow goes to EVE Frontier's
  official FusionAuth + Sui zkLogin endpoints.

## Attackers we consider

### In scope

1. **A compromised npm or Cargo dependency in the legitimate
   signed binary.** Closed by the [explicit CSP] in
   `tauri.conf.json`: the WebView can only `connect-src` to
   `'self'` + Tauri's IPC bridge — no exfiltration to attacker
   infrastructure even with arbitrary code-execution in the
   webview. Backend deps are smaller surface; `cargo audit` runs
   on every push.
2. **A malicious upstream Brave / EVE Vault / Sandboxie release.**
   - EVE Vault: SHA-256 verified against the upstream
     `checksums.txt` when published (best-effort warning when not).
   - Brave + Sandboxie: trusted by signature of the publisher's
     own signed installer; Bifrost only invokes their public
     download URLs and runs their installers, not arbitrary code.
3. **A network adversary** modifying GitHub Releases responses
   in flight. Caught by the minisign signature embedded in
   `latest.json`; auto-updater rejects mismatches.
4. **A user pointing Bifrost at a malicious Sandboxie path or
   game exe via the config file.** Defence: every shell-out passes
   the path through `cmd.args(...)`; `commands::config` validates
   the path exists before persisting.
5. **A non-Bifrost Brave process holding files locked during an
   in-place install** (the v0.0.4 Brave-Origin recovery scenario).
   Defence: install/uninstall pre-flight refuses if any
   Bifrost-managed `brave.exe` is running (filtered by
   `ExecutablePath` so the user's daily-driver Brave is ignored).

### Explicitly out of scope

1. **A compromised host machine.** If the user's Windows account is
   already controlled by the attacker, Bifrost can't defend
   anything — including the EVE Vault wallet session in the per-
   Rider Brave profile.
2. **A Sandboxie escape.** Bifrost depends on Sandboxie's isolation
   guarantees. Report Sandboxie vulnerabilities directly to
   [sandboxie-plus](https://github.com/sandboxie-plus/Sandboxie).
3. **Compromise of the maintainer's GitHub account or signing-key
   storage.** Mitigations: the signing key lives in a tag-gated
   GitHub Environment, not a repo-wide secret (PR-triggered runs
   cannot exfiltrate it). The branch-protection ruleset on `main`
   blocks force-pushes and deletions. No published key-rotation
   procedure yet — that's tracked as a pre-1.0 TODO.
4. **The Sui mainnet RPC endpoint serving false balance data.**
   Bifrost reads-only; a tampered balance is a display issue, not
   a financial-loss vector (the user's wallet is untouched).

## Non-goals

- **Bifrost is not a wallet.** It does not hold, transmit, sign,
  or display private keys at any layer. The EVE Vault extension
  is the wallet; Bifrost just makes sure the extension is
  installed in each per-Rider browser profile.
- **Bifrost is not a TOS-bypass.** No DLL injection, no input
  multiplexing, no reverse-engineered protocols. It drives only
  what Fenris Creations (formerly CCP Games) and Sandboxie have
  publicly documented.
- **Bifrost does not phone home.** No analytics, no telemetry, no
  crash reporters. Network calls listed exhaustively in
  `docs/ARCHITECTURE.md`. A planned **diagnostics panel** (v0.1.0
  scope) will surface this list in-app so users can verify it
  themselves.

## Reporting issues

Security issues: see [SECURITY.md](../SECURITY.md). Do not file
public GitHub issues for vulnerabilities.

## Document maintenance

This file should be reviewed any time:
- A new outbound network host is added
- The Tauri capability set changes
- A new bundled component / dependency is added (Brave, EVE Vault, …)
- The signing-key rotation procedure changes
- The CSP weakens
- A new asset class enters Bifrost's storage

Owners: the maintainer listed in `Cargo.toml`. Last reviewed: 2026-05-25.

[Sandboxie]: https://github.com/sandboxie-plus/Sandboxie
[Brave]: https://github.com/brave/brave-browser
[EVE Vault]: https://github.com/evefrontier/evevault
[explicit CSP]: ../src-tauri/tauri.conf.json
