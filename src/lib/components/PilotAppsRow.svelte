<script lang="ts">
  // Companion-site shortcut row for a single pilot. Extracted from
  // PilotCard.svelte specifically so it can be dynamically imported
  // — its dependencies (`faviconStore` + the favicon-fetching IPC
  // pipeline + companion-site icon loading) are only valuable when
  // the user has both Brave AND EVE Vault installed. For pilots-
  // only users (game launch via Sandboxie, no wallet workflow),
  // this entire chunk stays unloaded for the session.
  //
  // The lazy boundary is enforced by PilotCard: it only imports
  // this module when `integrationReady()` becomes true AND the
  // user has at least one enabled companion site.
  import type { Pilot } from "../types";
  import { configStore } from "../stores/config.svelte";
  import { faviconStore } from "../stores/favicons.svelte";
  import { pilotStore } from "../stores/pilots.svelte";
  import { api } from "../tauri";
  import { formatBackendError } from "../error";

  interface Props {
    pilot: Pilot;
  }

  let { pilot }: Props = $props();

  async function openApp(url: string) {
    try {
      await api.openPilotApp(pilot.id, url);
    } catch (e) {
      pilotStore.error = formatBackendError(e);
    }
  }

  // Lazy-load favicons for each enabled companion site. The store
  // dedupes by URL so repeat calls across pilot cards collapse to a
  // single IPC per host. Failed fetches cache as `null` and don't
  // retry within the session.
  $effect(() => {
    for (const site of configStore.enabledSites) {
      faviconStore.load(site.url);
    }
  });
</script>

<!-- Apps row — companion site shortcuts. Each tile launches a new
     per-pilot browser window with EVE Vault preloaded; on first use
     the user logs in via the extension's own flow, after which
     subsequent clicks just open the site as that pilot. -->
<div
  class="flex flex-wrap items-center gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg)]/20 px-4 py-2"
>
  {#each configStore.enabledSites as site (site.url)}
    {@const favicon = faviconStore.cache.get(site.url)}
    <button
      class="mono group flex h-11 w-11 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] bg-transparent text-[calc(12px*var(--text-scale,1))] font-bold text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)]"
      onclick={() => openApp(site.url)}
      title="{site.name} — {site.url}"
      aria-label="Open {site.name} as {pilot.name}"
    >
      {#if favicon}
        <img
          src={favicon}
          alt=""
          class="h-6 w-6 object-contain"
          draggable="false"
        />
      {:else}
        {site.icon}
      {/if}
    </button>
  {/each}
</div>
