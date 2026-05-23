<script lang="ts">
  import { onMount } from "svelte";
  import { pilotStore } from "../stores/pilots.svelte";
  import { vaultStore } from "../stores/vault.svelte";
  import { chromiumStore } from "../stores/chromium.svelte";
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
   *  outside-Bifrost events (game crashed, user launched a sandbox via
   *  the legacy .bat, etc.) without making the user click a Sync
   *  button. Cheap — one `Start.exe /pids:<box>` shellout per pilot. */
  const RECONCILE_INTERVAL_MS = 30_000;

  onMount(() => {
    // First-load probe in TWO phases:
    //   1. `refresh()` (inside bootstrap) — fast in-memory read of
    //      pilots.json, paints the roster in the first frame
    //   2. `reconcile_pilots` (inside bootstrap) — slow Sandboxie
    //      shellouts, runs in the background; `syncing` is true
    //      during this window so the UI can disable Launch/Stop
    //      and show a thin "Validating sandboxes…" bar
    // The 30 s background tick keeps using plain reconcile() — it
    // doesn't need to gate the UI because by then statuses are
    // already trustworthy.
    pilotStore.bootstrap();
    // Wallet integration is "ready" when BOTH Brave (the bundled
    // browser) AND the EVE Vault extension are installed. PilotCard
    // gates its Apps row on `integrationReady()`, which reads both
    // stores — they BOTH need a fresh probe on cold start, otherwise
    // the row stays hidden until something else (e.g. the user
    // visiting Settings) triggers the chromiumStore refresh.
    vaultStore.refresh();
    chromiumStore.refresh();
    // Config drives the per-pilot Apps row (companion sites list).
    configStore.refresh();
    // Host-detection state — drives the "Setup required" banner above
    // the roster when Sandboxie or the EVE Frontier client is missing.
    statusStore.refresh();

    // Background drift-correction. Lifecycle actions (Start / Stop /
    // Archive / Adopt) already trigger a refresh via the store's
    // `run()` wrapper, so this only catches state that changes outside
    // Bifrost's own actions.
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

  /** Pilot-card grid template, driven by Settings › Display › Roster
   *  layout. Auto (0) keeps the original `auto-fit` behaviour where
   *  cards stay at 280–320 px and wrap freely; 2 / 3 lock the row
   *  count and let cards stretch to fill the window so the user gets
   *  consistent geometry across resizes. Same value is reused for
   *  Discovered + Archived grids so all three sections stay in sync.
   *  `minmax(0, 1fr)` (not `minmax(280px, 1fr)`) so the locked layouts
   *  remain valid when the window is narrower than the natural
   *  280 px × N — better to compress cards than to overflow. */
  const rosterGridStyle = $derived.by(() => {
    const cols = configStore.config?.rosterColumns ?? 0;
    if (cols === 2 || cols === 3) {
      return `grid-template-columns: repeat(${cols}, minmax(0, 1fr));`;
    }
    return "grid-template-columns: repeat(auto-fit, minmax(280px, 320px));";
  });

  /** Outer-container width cap, also driven by the Roster layout
   *  setting. When the user picks Auto we drop the `max-w-6xl`
   *  (1152 px) ceiling so dragging the window wider keeps adding
   *  columns — 4, 5, 6 pilots per row, as far as their monitor goes.
   *  The fixed presets (2 / 3) keep the ceiling because the whole
   *  point of those choices is a stable layout the user has tuned
   *  the window to fit; letting them expand unbounded would defeat
   *  the lock.
   *
   *  Returns a Tailwind class string rather than a style attribute
   *  so the rest of the container's utility classes stay in the
   *  same idiom. */
  const containerWidthClass = $derived(
    (configStore.config?.rosterColumns ?? 0) === 0 ? "" : "max-w-6xl",
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
<div class="mx-auto flex w-full {containerWidthClass} flex-col gap-4">
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
       with `text-[11px]` leading + py-2.5 + border) and the h1 alone
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
          class="mono w-[220px] border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-[calc(12px*var(--text-scale,1))] text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
        />
        <Button onclick={handleCreate} disabled={creating || !newName.trim()}>
          {creating ? "Creating…" : "+ Add pilot"}
        </Button>
      </div>
    </div>
  </header>

  <div class="flex flex-col gap-8 px-6 pb-8">
    <!-- Cold-start sync indicator. Visible only during the
         `bootstrap()` second phase (cached roster already on screen,
         reconcile in flight). Thin animated bar in the accent colour
         + a single line of label text — present enough to explain
         the brief Launch-button disable, quiet enough that it doesn't
         compete with the roster itself. Hides automatically once
         reconcile completes, typically within 1–3 s. -->
    {#if pilotStore.syncing && managedPilots.length > 0}
      <div class="flex items-center gap-3 border border-[var(--color-border)] bg-[var(--color-surface)]/40 px-4 py-2">
        <span
          class="mono text-[calc(10px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-text-muted)] uppercase"
        >
          Validating sandboxes…
        </span>
        <div class="relative h-[2px] flex-1 overflow-hidden bg-[var(--color-border)]">
          <div class="sync-bar absolute top-0 left-0 h-full w-1/3" style:background="var(--color-accent)"></div>
        </div>
      </div>
    {/if}

    <!-- Managed -->
    <div class="flex flex-col gap-4">
      {#if pilotStore.loading && managedPilots.length === 0}
        <div class="text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-muted)] tracking-[0.2em] uppercase">
          Loading roster…
        </div>
      {:else if managedPilots.length === 0}
        <div
          class="flex flex-col items-center justify-center gap-2 border border-dashed border-[var(--color-border)] bg-[var(--color-surface)]/20 py-14 text-center"
        >
          <p class="text-[calc(11px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-muted)] uppercase">
            No pilots assigned
          </p>
          <p class="text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-dim)]">
            Designate a new pilot above, or adopt an existing sandbox below.
          </p>
        </div>
      {:else}
        <div class="grid justify-center gap-4" style={rosterGridStyle}>
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
            <span class="text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-dim)] uppercase">
              Sandboxes not yet under Bifrost control
            </span>
          </div>
          <span class="mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)]">
            {String(pilotStore.discovered.length).padStart(2, "0")}
          </span>
        </div>
        <div class="grid justify-center gap-4" style={rosterGridStyle}>
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
            <span class="text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-dim)] uppercase">
              Stashed · sandbox preserved · restorable
            </span>
          </div>
          <span class="mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)]">
            {String(archivedPilots.length).padStart(2, "0")}
          </span>
        </div>
        <div class="grid justify-center gap-4" style={rosterGridStyle}>
          {#each archivedPilots as pilot (pilot.id)}
            <ArchivedCard {pilot} />
          {/each}
        </div>
      </div>
    {/if}
  </div>
  </section>
</div>

<style>
  /* Indeterminate progress bar: a 1/3-width sliver slides left-to-right
     across the track on a 1.4 s loop. Keyframes go from -33 % (just off
     the left edge) to 100 % (fully off the right), so the leading edge
     enters and the trailing edge exits without ever sitting still. */
  @keyframes sync-sweep {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(300%);
    }
  }
  .sync-bar {
    animation: sync-sweep 1.4s ease-in-out infinite;
  }
</style>
