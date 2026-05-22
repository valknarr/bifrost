# Security Policy

Thank you for taking the time to make Bridge safer.

## Reporting a vulnerability

**Please do not report security issues through public GitHub issues.**

Email security reports privately to **valknarr@pm.me** with:

- A description of the issue and its impact (what an attacker could do).
- Step-by-step reproduction. Bridge builds in a few minutes from source —
  if you can include a minimal sample profile or scripted repro, that
  shortens triage time significantly.
- The Bridge version (`bridge --version` or the value in `Cargo.toml`),
  your Windows build, and which Sandboxie variant + version you're on
  (Plus or Classic).
- Whether you'd like to be credited in the release notes when the fix
  ships, and how (handle / email / silence).

We aim to acknowledge reports within **72 hours** and ship a fix within
**14 days** for high-severity issues. We will keep you in the loop on
progress.

## What we consider a vulnerability

Bridge installs a kernel driver (via Sandboxie's installer — Plus or
Classic) and spawns sandboxed game clients with elevated privileges.
The threat model takes that responsibility seriously. Examples of
in-scope issues:

- **Sandbox escape** — anything that causes Bridge to write the wrong
  isolation rules to `Sandboxie.ini`, or that allows code inside a
  pilot's sandbox to read or write data belonging to another pilot.
- **Privilege escalation** — Bridge runs the Sandboxie installer
  under UAC; any path where untrusted input influences the installer
  command line or where the elevated process can be coerced into running
  arbitrary code.
- **Wallet / identity leakage** — anything that allows one pilot's
  Chromium profile (and therefore EVE Vault session) to leak into
  another pilot's session, or to the host's day-to-day browser.
- **Untrusted download paths** — if Bridge can be tricked into
  downloading a substitute for Brave, EVE Vault, or the Sandboxie
  installer from somewhere other than the canonical GitHub releases.
- **Code execution from observed content** — Bridge respects the
  prompt-injection / observed-content rules described in
  [CONTRIBUTING.md](./CONTRIBUTING.md); deviations are bugs.

## Current security posture

- **Webview content origin.** Bridge's Tauri webview only loads its
  own bundled frontend (`../dist`). All remote content (companion-site
  favicons, GitHub releases, Sui RPC) is fetched through the Rust
  backend via the shared HTTP client and returned to the frontend
  as data URLs / parsed structs. The webview itself never makes
  direct outbound requests.
- **Content Security Policy.** Set to `null` in `tauri.conf.json` —
  deliberate while we baseline the design (Tailwind 4 injects styles
  inline at build, the favicon path serves `data:image/png;base64,…`
  URLs into `<img>` tags, the IPC bridge needs `ipc://` and
  `tauri://` schemes). A restrictive explicit CSP is on the README
  roadmap; the current `null` setting is acceptable for a v0.0.1
  desktop app with no remote-content surface, but tightening it
  before any plugin sandbox / extension hosting work is prudent.
- **Per-pilot browser.** Brave runs *inside* a Sandboxie box AND under
  its own per-pilot `--user-data-dir`. Cross-pilot session leakage
  would require either a Sandboxie escape or Bridge writing the wrong
  profile path; the second is covered by the cascading integration
  tests pinning the `prepare_profile` contract.

## Out of scope

- **Sandboxie vulnerabilities** (Plus or Classic) — please report those
  to
  [`sandboxie-plus/Sandboxie`](https://github.com/sandboxie-plus/Sandboxie/security/policy)
  directly. Bridge depends on Sandboxie's isolation guarantees and will
  track its security advisories, but the underlying kernel driver is
  not in our maintenance footprint.
- **EVE Vault vulnerabilities** — please report those to
  [`evefrontier/evevault`](https://github.com/evefrontier/evevault).
  Bridge bundles the extension verbatim from upstream.
- **Brave Browser vulnerabilities** — please report those to
  [Brave's HackerOne program](https://hackerone.com/brave).

## Disclosure timeline

Once a fix has shipped to a stable release, we publish an advisory in
the GitHub Security Advisories tab of this repo and credit the reporter
(with their permission). For high-severity issues we coordinate a
joint-disclosure timeline with the reporter.
