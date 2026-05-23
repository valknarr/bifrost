// Auto-update flow. Wraps `@tauri-apps/plugin-updater` so the UI
// doesn't need to know about plugin internals.
//
// Lifecycle:
//   1. `check()` on app mount (debounced by Tauri-side cache) →
//      polls the signed manifest at
//      `tauri.conf.json::plugins.updater.endpoints[0]`.
//   2. If an update is available, `available` flips true and the
//      `UpdateBanner` component appears.
//   3. User clicks "Restart and update" → `installAndRelaunch()`
//      downloads, verifies the embedded signature against the
//      pubkey baked into the .exe at build time, runs the new
//      installer in passive mode, and relaunches.
//   4. Failures (signature mismatch, network, etc.) surface as a
//      soft error in the banner — the app stays on the current
//      version.
//
// Signature verification happens in Rust before installer launch;
// if `pubkey` in tauri.conf.json was a build-time placeholder
// rather than a real key, EVERY update check will fail with a
// signature error. `docs/RELEASING.md` covers the one-time key-gen.

import type { Update } from "@tauri-apps/plugin-updater";

class UpdaterStore {
  /** Latest update metadata if one is available, null otherwise. */
  available = $state<Update | null>(null);

  /** Human-readable version string (e.g. "0.1.0") of the available
   *  update, surfaced in the banner. */
  get newVersion(): string | null {
    return this.available?.version ?? null;
  }

  /** Soft error from the last check / install attempt. Cleared on
   *  every fresh `check()`. */
  error = $state<string | null>(null);

  /** True while the user-triggered install is downloading + verifying. */
  installing = $state(false);

  /** Progress percent (0–100) for the active install. `null` when
   *  not installing. */
  progress = $state<number | null>(null);

  /** Poll the updater endpoint for a newer signed release. No-op if
   *  already checking. Errors are stashed on `this.error` rather
   *  than thrown — the banner is opt-in UI, never a hard failure. */
  async check() {
    this.error = null;
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        this.available = update;
      }
    } catch (e) {
      // Common failure modes here:
      //   - Pubkey placeholder still in tauri.conf.json (dev build)
      //   - No network
      //   - Manifest not yet published (first release pending)
      // All non-fatal; the app keeps working at its current version.
      const msg = e instanceof Error ? e.message : String(e);
      // Don't show the user a banner for these — log and move on.
      // Dev builds will see the pubkey error every launch otherwise.
      console.warn("updater: check failed:", msg);
      this.error = msg;
    }
  }

  /** Download + verify + run the new installer + relaunch. The
   *  installer runs in `passive` mode (see tauri.conf.json) so the
   *  user sees the Windows progress bar but doesn't have to click
   *  through wizard pages. */
  async installAndRelaunch() {
    if (!this.available || this.installing) return;
    this.installing = true;
    this.progress = 0;
    this.error = null;
    try {
      let downloaded = 0;
      let total = 0;
      await this.available.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength ?? 0;
            if (total > 0) {
              this.progress = Math.min(99, Math.round((downloaded / total) * 100));
            }
            break;
          case "Finished":
            this.progress = 100;
            break;
        }
      });
      // After downloadAndInstall, relaunch. Tauri's process plugin
      // handles the relaunch atomically — current window closes
      // before the new .exe starts so we don't get two instances.
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      this.error = `Update failed — ${msg}`;
      this.installing = false;
      this.progress = null;
    }
  }

  /** Dismiss the update banner without installing. The check
   *  re-runs on next launch, so the user sees the banner again
   *  unless they actually take the upgrade. */
  dismiss() {
    this.available = null;
    this.error = null;
  }
}

export const updaterStore = new UpdaterStore();
