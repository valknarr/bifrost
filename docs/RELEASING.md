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
   runs locally, then click **Publish release**.
7. Existing Bifrost users see the in-app banner on their next launch.

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
