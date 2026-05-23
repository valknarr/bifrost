<script lang="ts">
  // Companion-site shortcut row for a single rider. Extracted from
  // RiderCard.svelte specifically so it can be dynamically imported
  // — its dependencies (`faviconStore` + the favicon-fetching IPC
  // pipeline + companion-site icon loading) are only valuable when
  // the user has both Brave AND EVE Vault installed. For riders-
  // only users (game launch via Sandboxie, no wallet workflow),
  // this entire chunk stays unloaded for the session.
  //
  // The lazy boundary is enforced by RiderCard: it only imports
  // this module when `integrationReady()` becomes true AND the
  // user has at least one enabled companion site.
  import type { Rider } from "../types";
  import { configStore } from "../stores/config.svelte";
  import { faviconStore } from "../stores/favicons.svelte";
  import { riderStore } from "../stores/riders.svelte";
  import { api } from "../tauri";
  import { formatBackendError } from "../error";

  interface Props {
    rider: Rider;
  }

  let { rider }: Props = $props();

  async function openApp(url: string) {
    try {
      await api.openRiderApp(rider.id, url);
    } catch (e) {
      riderStore.error = formatBackendError(e);
    }
  }

  // Lazy-load favicons for each enabled companion site. The store
  // dedupes by URL so repeat calls across rider cards collapse to a
  // single IPC per host. Failed fetches cache as `null` and don't
  // retry within the session.
  $effect(() => {
    for (const site of configStore.enabledSites) {
      faviconStore.load(site.url);
    }
  });
</script>

<!-- Apps row — companion site shortcuts. Each tile launches a new
     per-rider browser window with EVE Vault preloaded; on first use
     the user logs in via the extension's own flow, after which
     subsequent clicks just open the site as that rider. -->
<div
  class="flex flex-wrap items-center gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg)]/20 px-4 py-2"
>
  {#each configStore.enabledSites as site (site.url)}
    {@const favicon = faviconStore.cache.get(site.url)}
    <button
      class="mono group flex h-11 w-11 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] bg-transparent text-[calc(12px*var(--text-scale,1))] font-bold text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)]"
      onclick={() => openApp(site.url)}
      title="{site.name} — {site.url}"
      aria-label="Open {site.name} as {rider.name}"
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
