# Releasing Bifrost

How to ship a new signed Bifrost build that existing users will pick up
via the in-app auto-updater.

## One-time setup — generate the updater signing key

The updater verifies every downloaded `.exe` against a public key
**baked into the previous build** at compile time. Without this signing
chain, a network attacker could trick users into installing a malicious
"update". Generate the keypair exactly once; the public key goes in the
repo, the private key stays in a password manager + GitHub Actions
secret.

```powershell
# From the repo root. Requires the Tauri CLI (already a devDependency).
pnpm tauri signer generate -w ~/.bifrost-updater.key

# The CLI prints:
#   - the public key (paste into tauri.conf.json — see step 2)
#   - the private key path (~/.bifrost-updater.key)
#   - the password you set during generation (KEEP)
```

**Treat the private key like a code-signing cert.** If it leaks, anyone
can ship a signed payload that existing Bifrost users will accept as a
legitimate update. If you lose it, you can never auto-update existing
installs — they'll be stuck on the version that has the old public key
embedded, and you'd have to ask every user to manually re-download.

### Step 1 — Replace the public-key placeholder

Open `src-tauri/tauri.conf.json` and replace:

```json
"pubkey": "PUBKEY_PLACEHOLDER_SEE_DOCS_RELEASING"
```

with the public key string `tauri signer generate` printed. Commit
this change in the same PR that turns on releases.

### Step 2 — Add the private key to the `release` environment

The signing key is the highest-value secret in this repo: anyone who
can read it can publish a malicious `.exe` that auto-updates onto
every Bifrost user's machine. Rather than storing it as a repo-wide
secret, gate it through a GitHub Environment so it's only readable
when the workflow is triggered by a release tag (not by a PR a
contributor opens against `main`).

1. In the repo, go to **Settings → Environments → New environment**
   and name it `release`. (Not `prod` — Bifrost isn't a server you're
   deploying to, you're cutting a signed artifact.)
2. Under **Deployment branches and tags**, switch to
   **Selected branches and tags** and add the rule `v*.*.*`. This is
   the security-critical bit: it means the secrets below are *only*
   readable when the workflow runs from a `v*.*.*` tag. A malicious
   PR opened against `main` that runs CI cannot exfiltrate the key by
   adding a step that echoes it, because PR-triggered runs are not in
   this environment.
3. Skip **Required reviewers** unless you're collaborating — for a
   solo project it's friction without a security improvement.
4. Under **Environment secrets**, add **two** secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` — the *contents* of `~/.bifrost-updater.key`
     (the full file, including the `untrusted comment:` header)
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password you set
     during generation. Use an empty string if you didn't set one.

The release workflow (`.github/workflows/release.yml`) declares
`environment: release` on the build job, so it claims these secrets
on tag-triggered runs and passes them to `tauri-action`, which signs
the build and emits a `latest.json` alongside the `.exe`.

## Per-release flow

1. Bump the version in **both** `package.json` and `src-tauri/Cargo.toml`
   to the new tag (e.g. `0.1.0`). Cargo's `tauri.conf.json` reads
   `version` from `Cargo.toml`, so they must match.
2. Update `CHANGELOG.md` — add an entry under `## [Unreleased]`, then
   rename it to `## [0.1.0] – YYYY-MM-DD`.
3. Commit: `git commit -am "chore(release): v0.1.0"`.
4. Tag: `git tag v0.1.0 && git push origin main --tags`.
5. The `release.yml` workflow fires automatically:
   - builds the .exe on `windows-latest`
   - signs the .exe + bundles using the secrets above
   - generates `latest.json` (manifest with version + download URL +
     signature)
   - creates a **draft** GitHub release with both artifacts attached
6. Review the draft release at
   `https://github.com/valknarr/bifrost/releases` — verify the `.exe`
   runs locally, then click **Publish release**. On the publish
   dialog, **PAY ATTENTION TO TWO TOGGLES** that GitHub defaults
   wrong for `v0.x.x` releases:
   - **"Set as a pre-release"** — make sure this is **UNCHECKED**.
     GitHub often pre-checks it for sub-1.0 versions assuming
     "initial development." Pre-release flag excludes the release
     from the `/releases/latest` resolution, which means the
     auto-updater's `latest/download/latest.json` URL returns
     404 and **the in-app banner never appears for any user**.
   - **"Set as the latest release"** — make sure this is selected
     (the dropdown has Latest / Pre-release / None). Tauri's
     updater polls the URL the `Latest` toggle controls.

   If you find an already-published release in the wrong state,
   fix it via `gh release edit vX.Y.Z --prerelease=false --latest`.
7. Existing Bifrost users see the in-app banner on their next launch.

## Release artifacts

Each release ships four files attached to the GitHub Release:

- `Bifrost_<ver>_x64-setup.exe` — the NSIS installer users download
- `Bifrost_<ver>_x64-setup.exe.sig` — standalone minisign signature
  of the `.exe`, verifiable with the
  [`minisign`](https://jedisct1.github.io/minisign/) CLI using the
  pubkey embedded in `tauri.conf.json::plugins.updater.pubkey`
  (also pinned at the bottom of [SECURITY.md](../SECURITY.md) for
  copy-paste).
- `latest.json` — auto-updater manifest. Contains version, download
  URL, and an embedded minisign signature over the `.exe`. The
  in-app updater verifies this automatically against the pubkey
  baked into the running binary.
- `SHA256SUMS.txt` — `<hash>  <filename>` per the GNU coreutils
  format. For users who'd rather verify integrity without a minisign
  install:
  ```powershell
  # Windows
  Get-FileHash Bifrost_<ver>_x64-setup.exe -Algorithm SHA256
  ```
  ```sh
  # Linux / macOS
  sha256sum -c SHA256SUMS.txt
  ```
  Until Authenticode signing lands (the v0.1.0-rc2 blocker), these
  two paths (`minisign` of the `.exe` AND the SHA256 line) are the
  manual-verification options. See `docs/THREAT_MODEL.md` for what
  this defence actually protects against.

## What the user experiences

- Bifrost polls `https://github.com/valknarr/bifrost/releases/latest/download/latest.json`
  on every cold start.
- If the manifest's version is newer than the installed version AND the
  signature verifies against the pubkey baked into the running .exe,
  the update banner appears at the top of the window.
- User clicks **Restart and update** → Bifrost downloads the new
  installer (with progress bar) → Windows runs the new installer in
  `passive` mode (no clicks needed, brief progress dialog) → Bifrost
  relaunches.
- User clicks **Later** → banner dismisses for the session, reappears
  on next launch.

## Common failure modes

- **"signature mismatch"** in the banner — your signing key changed
  since the user installed Bifrost (e.g. you generated a new key
  instead of recovering the original). You can't fix this remotely;
  affected users have to manually re-download the latest .exe from
  the Releases page once, after which auto-updates resume.
- **"could not deserialize: invalid type: null"** during dev — the
  `pubkey` field in `tauri.conf.json` is still the placeholder. Fine
  for development; replace before publishing.
- **`tauri build` fails locally with "no private key"** — you haven't
  set `TAURI_SIGNING_PRIVATE_KEY`. Local dev builds don't need to be
  signed; only the CI release build does. Make sure
  `createUpdaterArtifacts` is NOT in `tauri.conf.json` at the bundle
  level (the release workflow injects it).

## Verifying the manifest manually

After publishing a release, you can sanity-check the manifest with:

```powershell
# Replace the URL with the one in tauri.conf.json's `endpoints` array.
curl -sL https://github.com/valknarr/bifrost/releases/latest/download/latest.json
```

The response should be a JSON object with `version`, `pub_date`,
`platforms`, and signatures. If it 404s, the workflow ran but didn't
publish; if it's there but signatures are empty, the
`TAURI_SIGNING_PRIVATE_KEY` secret isn't set in GitHub.

---

## Key rotation runbook

The minisign signing private key is **the most security-sensitive
secret in this repo.** Anyone who reads it can publish a signed
release that every installed Bifrost will accept and auto-install
on next launch. This section is the recovery procedure if:

1. **The key file was lost** (laptop died, drive failed, you can't
   find the backup), OR
2. **The key was compromised** (machine breach, key leaked, a
   collaborator gained transient access).

The two cases differ in urgency — lost = inconvenient,
compromised = drop-everything-and-fix. The steps are the same; the
*announcement* differs.

### Step 1 — Generate the new key

```sh
pnpm tauri signer generate -w ~/.bifrost-updater-NEXT.key
```

Treat the prompt-supplied passphrase the same way you treated the
original (a password manager entry, ideally one that's NOT on the
compromised machine if this is the compromise path).

The command prints the new pubkey to stdout. **Keep both the new
private key file AND the printed pubkey somewhere safe before
moving on** — there's a window in the next steps where you'll
need both.

### Step 2 — Pin the new pubkey in `tauri.conf.json`

Open `src-tauri/tauri.conf.json`, find
`plugins.updater.pubkey`, and replace the value with the new
pubkey string (base64, no comment line — same format the file
already has).

```jsonc
"plugins": {
  "updater": {
    "pubkey": "RWNEW_KEY_HERE...",
    "endpoints": [...]
  }
}
```

### Step 3 — Update the pinned pubkey in `SECURITY.md`

Find the "Updater signing pubkey" section at the bottom of
`SECURITY.md` and replace the base64 value. Add a small "Key
rotation history" sub-section noting the date, the old pubkey, the
new pubkey, and which CHANGELOG version introduces the swap.

### Step 4 — Rotate the GitHub Environment secrets

Repo Settings → Environments → `release`:

- **Delete** the old `TAURI_SIGNING_PRIVATE_KEY` +
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets.
- **Add** the new ones (contents of the new `.key` file +
  the passphrase you set in Step 1).

The environment's `v*.*.*` deployment-tag rule stays the same.

### Step 5 — Cut a release with the new pubkey

Bump version per the normal per-release flow (recommended:
go straight to a meaningful version bump like `0.1.0` so the
rotation is announceable), update CHANGELOG, commit, tag, push.

CI builds and signs the new release with the NEW key. Existing
Bifrost installs (which still have the OLD pubkey baked in)
will receive `latest.json` signed with the new key on their next
cold-start poll — and **reject it** because the signature doesn't
match the pubkey they have.

The user-facing symptom is a logged "signature mismatch" error
from the updater plugin. No banner appears. The app keeps working
at the existing version; the user has no automatic path forward.

### Step 6 — Announce + provide a manual recovery path

Affected users (everyone on a release that predates the rotation)
must **manually download the new .exe from the Releases page once.**
After that re-install, their .exe has the new pubkey baked in and
auto-updates resume.

**Announcement channels in order:**

1. **CHANGELOG entry** in the rotation release with a `### Security`
   subheading describing what happened, why, and the one-time
   manual-download recovery step. Be explicit: *"users on
   v0.0.X-earlier must download the new .exe once; auto-updates
   resume after that re-install."*
2. **README's "Status" section** updated to call out the
   one-time recovery step at the top.
3. **GitHub Discussion pinned at the top** of the Discussions tab
   linking to the release notes and the recovery step.
4. **Direct outreach** if you have any Discord / EVE community
   channels where Bifrost users are concentrated.

If this was a compromise (not a loss), **publish a GitHub Security
Advisory** in the repo's Security tab as well, with:

- The time window the old key was potentially exposed.
- Whether any malicious release was actually pushed using the old
  key (check `gh release list` history; releases you didn't author
  are signed with the compromised key).
- The recovery step above.

### Step 7 — Revoke + destroy the old key

After the new release ships and the announcement is out, securely
delete the old `~/.bifrost-updater.key` file from any machine it
touched. Update any password-manager entries. If the key was on a
backup, scrub the backup.

### What the rotation costs the user

- **One manual download** from the Releases page (the same UX
  they had on first install).
- **One install dialog** (Windows SmartScreen + UAC for the
  Sandboxie kernel-driver step, if not already installed).
- **No data loss.** Per-Rider browser profiles, riders.json,
  config.json, and the favicon cache all live in app-data and
  survive the manual re-install.

### Why we can't auto-rotate

Because the very thing being rotated IS the trust anchor. The new
release would need to be signed with a key the user already trusts
— which means either the OLD key (defeating the rotation if it's
been compromised) or a key the user has somehow already received
out-of-band (which would itself be a TOFU problem). Sovereign
single-key updaters always have this manual-rotation tail; the
alternative is multi-key / threshold systems that are
disproportionate complexity for a project this size.
