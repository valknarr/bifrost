// App-level info that's stable for the lifetime of the running process:
// version, build date, name. Sourced from `tauri.conf.json` via
// `@tauri-apps/api/app::getVersion()`, fetched once on first read so we
// don't pay the IPC cost on every footer render.
//
// Kept separate from `statusStore` / `configStore` because those reflect
// live, mutable state ("what's the Sandboxie detection state right
// now?"). This one is immutable per process — fetch once, cache forever.

import { getVersion } from "@tauri-apps/api/app";

class AppStore {
  /** Version string from `tauri.conf.json`, e.g. "0.0.3". Empty until
   *  the first `load()` resolves; renders as an empty span in the
   *  footer for a few hundred ms on cold start, which is the right
   *  tradeoff vs. blocking app boot on the IPC roundtrip. */
  version = $state<string>("");

  private loadPromise: Promise<void> | null = null;

  /** Fetch the version from Tauri. Idempotent — concurrent callers
   *  share the same in-flight promise so we never double-fetch. */
  load(): Promise<void> {
    if (this.loadPromise) return this.loadPromise;
    this.loadPromise = (async () => {
      try {
        this.version = await getVersion();
      } catch (e) {
        // Tauri unavailable (running in a plain browser tab during
        // dev experiments?) — leave version empty. Don't surface
        // an error to the user; this is decorative.
        console.warn("appStore: getVersion failed:", e);
      }
    })();
    return this.loadPromise;
  }
}

export const appStore = new AppStore();
