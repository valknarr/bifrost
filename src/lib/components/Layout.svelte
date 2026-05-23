<script lang="ts">
  import type { Snippet } from "svelte";
  import { riderStore } from "../stores/riders.svelte";
  import { routeStore, type Route } from "../stores/route.svelte";
  import { statusStore } from "../stores/status.svelte";
  import UpdateBanner from "./UpdateBanner.svelte";

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();

  const navItems: { id: Route; label: string }[] = [
    { id: "riders", label: "Riders" },
    { id: "settings", label: "Settings" },
  ];

  // Tiny "!" badge on the Settings tab whenever a host dependency is
  // missing — so the user has a second, persistent breadcrumb back to
  // Settings even after dismissing/scrolling past the banner.
  const settingsAlert = $derived(statusStore.missingAny);

  const runningCount = $derived(
    riderStore.riders.filter((p) => p.status === "running").length,
  );
  const managedCount = $derived(
    riderStore.riders.filter((p) => !p.archived).length,
  );

  // Current local time, ticking every second, for the bottom status strip
  let now = $state(new Date());
  $effect(() => {
    const t = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(t);
  });
  const clock = $derived(
    now.toLocaleTimeString("en-GB", { hour12: false }),
  );
</script>

<div class="grid h-full grid-cols-1 grid-rows-[48px_1fr_28px]">
  <!-- Top bar — brand · nav · live status. Single row, no left rail. -->
  <header
    class="flex items-stretch justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)]/80"
  >
    <div class="flex items-stretch">
      <div
        class="flex items-center gap-2.5 border-r border-[var(--color-border)] px-4"
      >
        <div
          class="flex h-6 w-6 items-center justify-center border border-[var(--color-accent)]"
          style:box-shadow="0 0 10px -2px var(--color-accent)"
        >
          <span class="mono text-[calc(10px*var(--text-scale,1))] font-bold text-[var(--color-accent)]"
            >B</span
          >
        </div>
        <span
          class="text-[calc(11px*var(--text-scale,1))] tracking-[0.32em] uppercase text-[var(--color-text)]"
        >
          Bifrost
        </span>
      </div>
      <nav class="flex items-stretch">
        {#each navItems as item}
          <button
            class="relative cursor-pointer px-5 text-[calc(11px*var(--text-scale,1))] tracking-[0.25em] uppercase transition-colors {routeStore.current ===
            item.id
              ? 'text-[var(--color-focus)]'
              : 'text-[var(--color-text-muted)] hover:text-[var(--color-text)]'}"
            onclick={() => routeStore.go(item.id)}
          >
            <span class="inline-flex items-center gap-2">
              {item.label}
              {#if item.id === "settings" && settingsAlert}
                <span
                  class="flex h-[14px] w-[14px] items-center justify-center bg-[var(--color-danger)] text-[calc(9px*var(--text-scale,1))] font-bold text-[var(--color-bg)]"
                  style:box-shadow="0 0 6px var(--color-danger)"
                  aria-label="Setup required"
                  title="Setup required — host dependencies missing"
                >
                  !
                </span>
              {/if}
            </span>
            {#if routeStore.current === item.id}
              <span
                class="absolute right-3 bottom-0 left-3 h-[2px] bg-[var(--color-focus)]"
                style:box-shadow="0 0 8px var(--color-focus)"
              ></span>
            {/if}
          </button>
        {/each}
      </nav>
    </div>
    <!-- Right side: dual-state HUD indicator.
         Left half = rider lifecycle (Online / Standby).
         Right half = system health (Ready / Setup required).
         The visual rhythm mirrors EVE Frontier's in-game era/cycle
         indicators (and CradleOS's "STILLNESS · UTOPIA" header
         badge). The settings-tab `!` badge takes care of the
         actionable nudge; this one's the always-on at-a-glance. -->
    <div
      class="flex items-center gap-3 px-4 text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] uppercase"
    >
      <!-- Riders state -->
      <div class="flex items-center gap-2">
        <div
          class="h-1.5 w-1.5 {runningCount > 0
            ? 'bg-[var(--color-ok)]'
            : 'bg-[var(--color-text-dim)]'}"
          style:box-shadow={runningCount > 0
            ? "0 0 8px var(--color-ok)"
            : "none"}
        ></div>
        {#if runningCount > 0}
          <span class="text-[var(--color-ok)]"
            >Online · {runningCount}/{managedCount}</span
          >
        {:else}
          <span class="text-[var(--color-text-muted)]"
            >Standby · 0/{managedCount}</span
          >
        {/if}
      </div>

      <span class="text-[var(--color-text-dim)]">·</span>

      <!-- System state -->
      <div class="flex items-center gap-2">
        <div
          class="h-1.5 w-1.5 {settingsAlert
            ? 'bg-[var(--color-warn)]'
            : 'bg-[var(--color-ok)]'}"
          style:box-shadow={settingsAlert
            ? "0 0 6px var(--color-warn)"
            : "0 0 8px var(--color-ok)"}
        ></div>
        {#if settingsAlert}
          <span class="text-[var(--color-warn)]">Setup</span>
        {:else}
          <span class="text-[var(--color-text-muted)]">System</span>
        {/if}
      </div>
    </div>
  </header>

  <!-- Update advisory: renders only when the updater store has a
       new signed release pending. Sits above the main content so
       it's the first thing the user sees on cold launch when an
       update is ready. Self-hides when the user clicks Later or
       finishes the upgrade flow. -->
  <UpdateBanner />

  <!-- Main content -->
  <main class="overflow-y-auto px-8 py-6">
    {@render children()}
  </main>

  <!-- Bottom status strip — game-flavoured ticker -->
  <footer
    class="flex items-center justify-between border-t border-[var(--color-border)] bg-[var(--color-surface)]/80 px-5 text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] uppercase text-[var(--color-text-dim)]"
  >
    <div class="flex items-center gap-3">
      <span class="text-[var(--color-text-muted)]"
        >{routeStore.current === "riders"
          ? "Rider Roster"
          : "System Configuration"}</span
      >
    </div>
    <div class="flex items-center gap-3">
      {#if riderStore.error}
        <span class="text-[var(--color-danger)]"
          >⚠ {riderStore.error.slice(0, 80)}</span
        >
      {/if}
      <span class="mono text-[var(--color-text-muted)]">{clock}</span>
    </div>
  </footer>
</div>
