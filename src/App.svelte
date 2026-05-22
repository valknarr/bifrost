<script lang="ts">
  import { onMount } from "svelte";
  import BackgroundField from "./lib/components/BackgroundField.svelte";
  import Layout from "./lib/components/Layout.svelte";
  import PilotsView from "./lib/views/PilotsView.svelte";
  import SettingsView from "./lib/views/SettingsView.svelte";
  import { routeStore } from "./lib/stores/route.svelte";
  import { configStore } from "./lib/stores/config.svelte";
  import { applyZoom } from "./lib/zoom";

  // Re-apply the persisted zoom as soon as we have a config in hand.
  // The Layout's own mount hook already calls `configStore.refresh()`
  // for the companion-sites + zoom data — we just react to the field
  // landing, which happens once on cold start and again whenever the
  // user changes presets in Settings.
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
    }
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
