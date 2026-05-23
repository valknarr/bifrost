<script lang="ts">
  import { onMount } from "svelte";
  import BackgroundField from "./lib/components/BackgroundField.svelte";
  import Layout from "./lib/components/Layout.svelte";
  import PilotsView from "./lib/views/PilotsView.svelte";
  import SettingsView from "./lib/views/SettingsView.svelte";
  import { routeStore } from "./lib/stores/route.svelte";
  import { configStore } from "./lib/stores/config.svelte";
  import { api } from "./lib/tauri";
  import {
    applyZoom,
    applySavedWindowSize,
    onWindowResizeStable,
  } from "./lib/zoom";

  /** Debounce window: how long after the user stops resizing the
   *  window before we write the new size to disk. 500 ms is long
   *  enough that a quick adjustment doesn't trigger a save mid-drag,
   *  short enough that the saved value tracks reality if the user
   *  closes the app right after letting go of the resize handle. */
  const RESIZE_PERSIST_DELAY_MS = 500;

  // Re-apply the persisted zoom + Auto-mode window size as soon as
  // we have a config in hand. The Layout's own mount hook already
  // calls `configStore.refresh()` for the companion-sites + zoom
  // data — we just react to the field landing, which happens once
  // on cold start and again whenever the user changes presets in
  // Settings.
  onMount(async () => {
    await configStore.refresh();
    if (configStore.config) {
      // Best-effort: if the webview-zoom permission is missing or
      // setZoom throws, fall back to the 1.0 default and log the
      // reason for diagnosis. The Settings picker is the place users
      // actually see this kind of error inline; startup must not
      // block on it.
      try {
        await applyZoom(configStore.config.uiZoom);
      } catch (e) {
        console.warn("applyZoom on startup failed:", e);
      }

      // Auto roster mode: restore the user's last window size before
      // they see the UI. Only apply when actually in Auto — the
      // fixed presets (2 / 3) carry their own width and we'd
      // otherwise undo their snap-on-pick. Failures stay non-fatal:
      // the window just opens at the tauri.conf.json default.
      if (configStore.config.rosterColumns === 0) {
        try {
          await applySavedWindowSize(
            configStore.config.rosterWindowWidth,
            configStore.config.rosterWindowHeight,
          );
        } catch (e) {
          console.warn("applySavedWindowSize on startup failed:", e);
        }
      }
    }

    // Wire up persistence going forward: every time the user stops
    // resizing, capture the size and (if we're in Auto mode) save
    // it. The listener stays attached for the lifetime of the app;
    // Tauri tears it down on window close.
    //
    // We read `rosterColumns` from the live store inside the
    // callback rather than at attach time so toggling between Auto
    // and 2/3 in Settings flips persistence on/off without needing
    // to re-attach the listener.
    onWindowResizeStable(RESIZE_PERSIST_DELAY_MS, async ({ width, height }) => {
      if (configStore.config?.rosterColumns !== 0) return;
      try {
        await api.setRosterWindowSize(width, height);
        // Refresh local config so the next save sees the latest
        // value and the Settings view stays in sync if the user is
        // looking at it.
        configStore.config = {
          ...configStore.config,
          rosterWindowWidth: width,
          rosterWindowHeight: height,
        };
      } catch (e) {
        console.warn("setRosterWindowSize failed:", e);
      }
    });
  });
</script>

<!-- Ambient drift field, mounted once at the root so it persists
     across route changes (vs. inside Layout which re-mounts when the
     route changes). Painted behind everything via `-z-10`. -->
<BackgroundField />

<Layout>
  {#if routeStore.current === "pilots"}
    <PilotsView />
  {:else if routeStore.current === "settings"}
    <SettingsView />
  {/if}
</Layout>
