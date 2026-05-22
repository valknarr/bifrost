<script lang="ts">
  import { onMount } from "svelte";
  import { pilotStore } from "../stores/pilots.svelte";
  import { vaultStore } from "../stores/vault.svelte";
  import { configStore } from "../stores/config.svelte";
  import { statusStore } from "../stores/status.svelte";
  import PilotCard from "../components/PilotCard.svelte";
  import ArchivedCard from "../components/ArchivedCard.svelte";
  import DiscoveredCard from "../components/DiscoveredCard.svelte";
  import SetupBanner from "../components/SetupBanner.svelte";
  import Button from "../components/Button.svelte";

  let newName = $state("");
  let creating = $state(false);

  /** How often to silently re-check pilot status. Catches drift from
   *  outside-Bridge events (game crashed, user launched a sandbox via
   *  the legacy .bat, etc.) without making the user click a Sync
   *  button. Cheap — one `Start.exe /pids:<box>` shellout per pilot. */
  const RECONCILE_INTERVAL_MS = 30_000;

  onMount(() => {
    // First-load probe: mirror the persistent on-disk state to what
    // Sandboxie says is actually running right now.
    pilotStore.reconcile();
    // Vault status drives whether each pilot card shows the Apps row.
    vaultStore.refresh();
    // Config drives the per-pilot Apps row (companion sites list).
    configStore.refresh();
    // Host-detection state — drives the "Setup required" banner above
    // the roster when Sandboxie or the EVE Frontier client is missing.
    statusStore.refresh();

    // Background drift-correction. Lifecycle actions (Start / Stop /
    // Archive / Adopt) already trigger a refresh via the store's
    // `run()` wrapper, so this only catches state that changes outside
    // Bridge's own actions.
    const tick = setInterval(
      () => pilotStore.reconcile(),
      RECONCILE_INTERVAL_MS,
    );
    return () => clearInterval(tick);
  });

  const managedPilots = $derived(
    pilotStore.pilots.filter((p) => !p.archived),
  );
  const archivedPilots = $derived(
    pilotStore.pilots.filter((p) => p.archived),
  );

  async function handleCreate() {
    if (!newName.trim()) return;
    creating = true;
    try {
      await pilotStore.create(newName.trim());
      newName = "";
    } finally {
      creating = false;
    }
  }
</script>

<!-- View-level stack: setup banner (when applicable) + main panel.
     Same max-width as Settings so both pages feel like one product. -->
<div class="mx-auto flex w-full max-w-6xl flex-col gap-4">
  <SetupBanner />

  <!-- Window-style container, mirrors in-game panel chrome -->
  <section
    class="relative flex flex-col gap-6 border border-[var(--color-border)] bg-[var(--color-surface)]/40"
  >
  <!-- Panel header. 2 px accent-tinted bottom border echoes
       CradleOS's HUD-style section dividers; inner section labels
       below the header keep the lighter 1 px rule so the visual
       hierarchy reads top-down.
       `min-h-[40px]` on the inner row is set HIGHER than the natural
       height of either constraint — the Add Pilot Button (~38 px
       with text-[11px] leading + py-2.5 + border) and the h1 alone
       (~28 px). Both this and the equivalent rule in SettingsView
       pin to the same value so the header doesn't grow / shrink by
       a pixel or two when the user tabs between the views. -->
  <header class="border-b-2 border-[var(--color-accent)]/40 px-6 pt-5 pb-4">
    <div class="flex min-h-[40px] flex-wrap items-center justify-between gap-4">
      <h1 class="title-bracket whitespace-nowrap text-lg tracking-[0.05em] text-[var(--color-text)]">
        Pilot Roster
      </h1>
      <div class="flex items-center gap-2">
        <input
          type="text"
          placeholder="New pilot designation…"
          bind:value={newName}
          onkeydown={(e) => e.key === "Enter" && handleCreate()}
          class="mono w-[220px] border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-[12px] text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
        />
        <Button onclick={handleCreate} disabled={creating || !newName.trim()}>
          {creating ? "Creating…" : "+ Add pilot"}
        </Button>
      </div>
    </div>
  </header>

  <div class="flex flex-col gap-8 px-6 pb-8">
    <!-- Managed -->
    <div class="flex flex-col gap-4">
      {#if pilotStore.loading && managedPilots.length === 0}
        <div class="text-[11px] text-[var(--color-text-muted)] tracking-[0.2em] uppercase">
          Loading roster…
        </div>
      {:else if managedPilots.length === 0}
        <div
          class="flex flex-col items-center justify-center gap-2 border border-dashed border-[var(--color-border)] bg-[var(--color-surface)]/20 py-14 text-center"
        >
          <p class="text-[11px] tracking-[0.2em] text-[var(--color-text-muted)] uppercase">
            No pilots assigned
          </p>
          <p class="text-[11px] text-[var(--color-text-dim)]">
            Designate a new pilot above, or adopt an existing sandbox below.
          </p>
        </div>
      {:else}
        <div class="grid justify-center gap-4 grid-cols-[repeat(auto-fit,minmax(280px,320px))]">
          {#each managedPilots as pilot (pilot.id)}
            <PilotCard {pilot} />
          {/each}
        </div>
      {/if}
    </div>

    <!-- Discovered -->
    {#if pilotStore.discovered.length > 0}
      <div class="flex flex-col gap-4">
        <div class="flex items-baseline justify-between">
          <div class="flex items-baseline gap-3">
            <h2 class="section-label">Discovered</h2>
            <span class="text-[10px] tracking-[0.2em] text-[var(--color-text-dim)] uppercase">
              Sandboxes not yet under Bridge control
            </span>
          </div>
          <span class="mono text-[10px] text-[var(--color-text-dim)]">
            {String(pilotStore.discovered.length).padStart(2, "0")}
          </span>
        </div>
        <div class="grid justify-center gap-4 grid-cols-[repeat(auto-fit,minmax(280px,320px))]">
          {#each pilotStore.discovered as box (box.name)}
            <DiscoveredCard {box} />
          {/each}
        </div>
      </div>
    {/if}

    <!-- Archived -->
    {#if archivedPilots.length > 0}
      <div class="flex flex-col gap-4">
        <div class="flex items-baseline justify-between">
          <div class="flex items-baseline gap-3">
            <h2 class="section-label">Archived</h2>
            <span class="text-[10px] tracking-[0.2em] text-[var(--color-text-dim)] uppercase">
              Stashed · sandbox preserved · restorable
            </span>
          </div>
          <span class="mono text-[10px] text-[var(--color-text-dim)]">
            {String(archivedPilots.length).padStart(2, "0")}
          </span>
        </div>
        <div class="grid justify-center gap-4 grid-cols-[repeat(auto-fit,minmax(280px,320px))]">
          {#each archivedPilots as pilot (pilot.id)}
            <ArchivedCard {pilot} />
          {/each}
        </div>
      </div>
    {/if}
  </div>
  </section>
</div>
