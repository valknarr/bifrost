<script lang="ts">
  /**
   * Companion-sites configuration. Lets the user enable / disable the
   * built-in companion sites (which can't be removed, only hidden) and
   * add custom sites that appear in every pilot's Apps row.
   *
   * State lives entirely in `configStore`; this component is a thin
   * presentation layer over it. Extracted from SettingsView so the
   * Settings panel stays focused on installer + paths concerns and so
   * we have a single place to expand companion-site UX later (drag-to-
   * reorder, per-site overrides, etc.).
   */

  import { configStore } from "../stores/config.svelte";
  import Button from "./Button.svelte";
  import IconEye from "./IconEye.svelte";

  // Reactive aliases so the markup stays readable.
  const sites = $derived(configStore.sites);
  const configError = $derived(configStore.error);

  // "Add custom site" form state — lives here rather than in the parent
  // because nothing outside the form cares about the in-progress input.
  let newSiteName = $state("");
  let newSiteUrl = $state("");
  let newSiteIcon = $state("");
  let addingSite = $state(false);

  async function handleAddSite() {
    if (!newSiteName.trim() || !newSiteUrl.trim()) return;
    addingSite = true;
    try {
      await configStore.addSite(
        newSiteName.trim(),
        newSiteUrl.trim(),
        newSiteIcon.trim() || undefined,
      );
      // Only clear the inputs when the backend accepted the entry —
      // otherwise the error stays visible alongside the user's text
      // so they can fix and retry without re-typing.
      if (!configStore.error) {
        newSiteName = "";
        newSiteUrl = "";
        newSiteIcon = "";
      }
    } finally {
      addingSite = false;
    }
  }
</script>

<div class="flex flex-col gap-3">
  <div class="flex items-baseline justify-between">
    <h2 class="section-label">Companion sites</h2>
    <span
      class="text-[10px] text-[var(--color-text-dim)] tracking-[0.2em] uppercase"
    >
      Each pilot opens these with their wallet identity
    </span>
  </div>

  <div
    class="flex flex-col gap-3 border border-[var(--color-border)] bg-[var(--color-bg)] px-5 py-4"
  >
    <!-- Existing sites. Disabled rows fade to ~50% opacity so the
         user can see at a glance which are hidden on pilot cards;
         the eye toggle on the right flips disabled state. -->
    <ul class="flex flex-col gap-1.5">
      {#each sites as site (site.url)}
        <li
          class="flex items-center gap-3 border border-transparent px-2 py-1.5 transition-opacity hover:border-[var(--color-border)] {site.disabled
            ? 'opacity-50'
            : ''}"
        >
          <span
            class="mono flex h-6 w-6 shrink-0 items-center justify-center border border-[var(--color-border-hi)] text-[10px] font-bold text-[var(--color-text-muted)]"
          >
            {site.icon}
          </span>
          <span class="flex-1 text-[12px] text-[var(--color-text)]">
            {site.name}
          </span>
          <span
            class="mono text-[10px] text-[var(--color-text-dim)] truncate max-w-[280px]"
            title={site.url}
          >
            {site.url}
          </span>
          {#if site.builtin}
            <span
              class="text-[9px] tracking-[0.2em] text-[var(--color-text-dim)] uppercase shrink-0"
            >
              Built-in
            </span>
            <!-- Built-ins can't be removed but they can be hidden. -->
            <button
              class="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center border border-transparent text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-border-hi)] hover:text-[var(--color-focus)]"
              onclick={() =>
                configStore.setSiteDisabled(site.url, !site.disabled)}
              aria-label={site.disabled
                ? `Enable ${site.name}`
                : `Disable ${site.name}`}
              title={site.disabled
                ? `Show ${site.name} on pilot cards`
                : `Hide ${site.name} from pilot cards`}
            >
              <IconEye hidden={site.disabled} />
            </button>
          {:else}
            <button
              class="cursor-pointer border border-transparent px-2 py-0.5 text-[10px] tracking-[0.15em] text-[var(--color-text-muted)] uppercase hover:border-[var(--color-danger)] hover:text-[var(--color-danger)]"
              onclick={() => configStore.removeSite(site.url)}
              title="Remove this site"
            >
              Remove
            </button>
          {/if}
        </li>
      {/each}
    </ul>

    <div class="h-px bg-[var(--color-border)]"></div>

    <!-- Add new site -->
    <div class="flex flex-col gap-2">
      <span
        class="text-[10px] tracking-[0.2em] text-[var(--color-text-muted)] uppercase"
      >
        Add custom site
      </span>
      <div class="grid grid-cols-[1fr_2fr_60px_auto] gap-2">
        <input
          type="text"
          placeholder="Name"
          bind:value={newSiteName}
          onkeydown={(e) => e.key === "Enter" && handleAddSite()}
          class="mono border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-[11px] text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
        />
        <input
          type="text"
          placeholder="https://example.com/"
          bind:value={newSiteUrl}
          onkeydown={(e) => e.key === "Enter" && handleAddSite()}
          class="mono border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-[11px] text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
        />
        <input
          type="text"
          placeholder="Icon"
          maxlength="2"
          bind:value={newSiteIcon}
          onkeydown={(e) => e.key === "Enter" && handleAddSite()}
          class="mono border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-center text-[11px] text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
        />
        <Button
          size="sm"
          disabled={addingSite || !newSiteName.trim() || !newSiteUrl.trim()}
          onclick={handleAddSite}
        >
          {addingSite ? "Adding…" : "+ Add"}
        </Button>
      </div>
      {#if configError}
        <div
          class="border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-3 py-2 text-[10px] text-[var(--color-danger)]"
        >
          {configError}
        </div>
      {/if}
    </div>
  </div>
</div>
