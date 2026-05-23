<script lang="ts">
  import { onMount } from "svelte";
  import type { Component } from "svelte";
  import BackgroundField from "./lib/components/BackgroundField.svelte";
  import Layout from "./lib/components/Layout.svelte";
  import PilotsView from "./lib/views/PilotsView.svelte";
  // SettingsView is loaded on demand — see the `$effect` below.
  // Cold-start frame skips parsing ~30 KB of Settings code (UI scale
  // picker, roster layout, companion-site CRUD, installer panels) for
  // every user, even the ones who never open Settings in a session.
  import { routeStore } from "./lib/stores/route.svelte";
  import { configStore } from "./lib/stores/config.svelte";
  import { updaterStore } from "./lib/stores/updater.svelte";
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

  /** Lazy-loaded `SettingsView` component. `null` until the user
   *  first navigates to the Settings tab; the dynamic `import`
   *  resolves the chunk, populates this state, and Svelte renders
   *  it. Once loaded, stays cached for the rest of the session. */
  let SettingsViewComponent = $state<Component | null>(null);

  /** Lazy-load the Settings code chunk the first time the user
   *  navigates there. After this `$effect` fires once, subsequent
   *  visits use the cached module. The brief delay (a few hundred
   *  ms over the IPC bridge on cold disks) is acceptable — Settings
   *  isn't on the critical path of app launch. */
  $effect(() => {
    if (routeStore.current === "settings" && !SettingsViewComponent) {
      import("./lib/views/SettingsView.svelte").then((m) => {
        SettingsViewComponent = m.default;
      });
    }
  });

  // Re-apply the persisted zoom + Auto-mode window size as soon as
  // we have a config in hand. The Layout's own mount hook already
  // calls `configStore.refresh()` for the companion-sites + zoom
  // data — we just react to the field landing, which happens once
  // on cold start and again whenever the user changes presets in
  // Settings.
  onMount(async () => {
    await configStore.refresh();
    if (configStore.config) {
      // Best-effort: if applying the persisted text-scale fails
      // (CSSOM unreachable, document not yet attached, etc.), fall
      // back to the 1.0 default and log the reason for diagnosis.
      // The Settings picker is the place users actually see this
      // kind of error inline; startup must not block on it.
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

    // Background: ask GitHub Releases (via the bundled pubkey) if a
    // newer signed Bifrost is available. Fire-and-forget; the
    // `UpdateBanner` component reacts to `updaterStore.available`
    // landing. On dev builds (pubkey placeholder) this fails
    // silently — see updater.svelte.ts for the warning log.
    updaterStore.check();
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
    <!-- SettingsView is dynamically imported on first navigation
         (see the `$effect` above). The brief null-state renders a
         lightweight loading hint instead of a hard blank. -->
    {#if SettingsViewComponent}
      {@const Comp = SettingsViewComponent}
      <Comp />
    {:else}
      <div
        class="mx-auto flex w-full max-w-6xl items-center justify-center px-6 py-12"
      >
        <span
          class="mono text-[calc(11px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-text-muted)] uppercase"
        >
          Loading settings…
        </span>
      </div>
    {/if}
  {/if}
</Layout>
