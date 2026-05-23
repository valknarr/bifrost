<script lang="ts">
  import { onMount } from "svelte";
  import type { Component } from "svelte";
  import type { Rider } from "../types";
  import { riderStore } from "../stores/riders.svelte";
  import { integrationReady } from "../stores/vault.svelte";
  import { configStore } from "../stores/config.svelte";
  import { clockStore, useClockTick } from "../stores/clock.svelte";
  import { api } from "../tauri";
  import { confirmDestructive } from "../confirm";
  import Button from "./Button.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import RiderPortrait from "./RiderPortrait.svelte";
  import IconArchive from "./IconArchive.svelte";
  // RiderAppsRow is intentionally NOT statically imported — its
  // companion-site icon + favicon-fetch code (the `faviconStore`
  // + IPC pipeline + `open_rider_app` flow) is only valuable when
  // Brave AND EVE Vault are both installed. For riders-only users
  // the chunk never loads. See the dynamic-import `$effect` below.

  // Subscribe to the shared clock tick so `balanceAge` re-derives
  // every 15 s. Without this the "Updated 5m ago" label would be
  // baked in at mount time and never update unless the rider record
  // changed for some other reason.
  useClockTick();

  interface Props {
    rider: Rider;
  }

  let { rider }: Props = $props();

  let pickingAccent = $state(false);
  let palette = $state<string[]>([]);

  onMount(async () => {
    try {
      palette = await api.getAccentPalette();
    } catch {
      // Backend not ready or call failed — picker just falls back to the
      // current accent and gracefully renders no swatches.
      palette = [];
    }
  });

  async function pickAccent(color: string) {
    await riderStore.setAccent(rider.id, color);
    pickingAccent = false;
  }

  /** Lazily-imported `RiderAppsRow.svelte` component, populated by
   *  the `$effect` below the first time wallet integration becomes
   *  available. Until then this stays `null` and the entire
   *  companion-site + favicon code path is unloaded. */
  let AppsRowComponent = $state<Component<{ rider: Rider }> | null>(null);

  /** Trigger the Apps-row chunk download the moment integration
   *  becomes ready AND there's at least one enabled companion site.
   *  Both checks gate the import together so a user with Brave +
   *  EVE Vault installed but every site disabled still doesn't pay
   *  the bytes. */
  $effect(() => {
    if (
      !AppsRowComponent &&
      integrationReady() &&
      configStore.enabledSites.length > 0
    ) {
      import("./RiderAppsRow.svelte").then((m) => {
        AppsRowComponent = m.default;
      });
    }
  });

  const shortAddr = $derived(
    rider.walletAddress
      ? `${rider.walletAddress.slice(0, 6)}…${rider.walletAddress.slice(-4)}`
      : "—",
  );

  // Inline wallet editor state
  let editingWallet = $state(false);
  let walletInput = $state("");
  let savingWallet = $state(false);

  function startEditWallet() {
    walletInput = rider.walletAddress ?? "";
    editingWallet = true;
  }

  async function saveWallet() {
    savingWallet = true;
    try {
      await riderStore.setWallet(rider.id, walletInput.trim());
      editingWallet = false;
    } finally {
      savingWallet = false;
    }
  }

  function cancelEditWallet() {
    editingWallet = false;
    walletInput = "";
  }
  const shortSandbox = $derived(
    rider.sandbox.length > 18
      ? `${rider.sandbox.slice(0, 8)}…${rider.sandbox.slice(-4)}`
      : rider.sandbox,
  );

  const isRunning = $derived(rider.status === "running");
  const isBusy = $derived(rider.status === "starting");
  const isMissing = $derived(rider.status === "missing");
  /** While cold-start sync is in flight, gate Launch/Stop. The cached
   *  status badge may be stale, and clicking Launch on a rider whose
   *  game is actually still running would spawn a second instance. */
  const syncing = $derived(riderStore.syncing);
  const showFirstLaunchHint = $derived(
    !rider.launchedAtLeastOnce && !isRunning && !isMissing,
  );

  /** Balance staleness — drives the small label below the stats grid
   *  that tells the user when figures were last refreshed from the
   *  Sui RPC. Three tiers, mapped from the age of
   *  `wallet_balance_fetched_at`:
   *
   *  * `null` (no successful fetch yet or no wallet address) — no
   *    label shown
   *  * < 60 s — fresh, dim "Updated just now"
   *  * 60 s ≤ age < 5 min — "Updated 2m ago", neutral muted colour
   *  * ≥ 5 min — "Updated 12m ago — stale", warn-yellow tint
   *
   *  Recomputes every 15 s via `clockStore.now`. Without the explicit
   *  read of `clockStore.now` inside this `$derived`, the value
   *  freezes at mount time because nothing else changes in the
   *  dependency graph between refreshes. */
  const balanceAge = $derived.by(() => {
    if (!rider.walletAddress) return null;
    const fetchedAt = rider.walletBalanceFetchedAt;
    if (fetchedAt == null) return null;
    const ageMs = clockStore.now - fetchedAt;
    if (ageMs < 0) return null; // clock skew / future timestamp — ignore
    const ageSeconds = Math.floor(ageMs / 1000);
    const ageMinutes = Math.floor(ageSeconds / 60);
    if (ageSeconds < 60) {
      return { label: "Updated just now", stale: false };
    }
    if (ageMinutes < 5) {
      return { label: `Updated ${ageMinutes}m ago`, stale: false };
    }
    if (ageMinutes < 60) {
      return { label: `Updated ${ageMinutes}m ago · stale`, stale: true };
    }
    const ageHours = Math.floor(ageMinutes / 60);
    return { label: `Updated ${ageHours}h ago · stale`, stale: true };
  });

  /** Confirm + permanently delete a rider whose sandbox is gone. The
   *  backend bypasses the normal "archive-first" guard for Missing
   *  riders so a single click is enough. We still confirm because the
   *  rider record (wallet address, accent, custom name) goes with it. */
  async function removeMissing() {
    const ok = confirmDestructive(
      `Remove rider "${rider.name}"?\n\nIts Sandboxie sandbox is gone, so there's nothing to clean up there. Bifrost will forget this rider's wallet, accent, and browser profile. This cannot be undone.`,
    );
    if (!ok) return;
    await riderStore.deletePermanently(rider.id);
  }

  // Favicon prefetching used to live here as a `$effect` over
  // `configStore.enabledSites`. It moved into `RiderAppsRow.svelte`
  // along with the rest of the wallet-integration UI so the
  // `faviconStore` IPC pipeline is only loaded when the user
  // actually has the wallet workflow installed.
</script>

<article
  class="group relative flex flex-col bg-[var(--color-surface)]/60 transition-colors {isRunning
    ? 'border border-[var(--color-focus)]/60'
    : 'border border-[var(--color-border)] hover:border-[var(--color-focus)]/60'}"
  style:--rider-accent={rider.accent}
>
  <!-- Outer corner brackets (in-game window-corner motif) -->
  <div
    class="pointer-events-none absolute -top-px -left-px h-2.5 w-2.5 border-t border-l"
    style:border-color={rider.accent}
  ></div>
  <div
    class="pointer-events-none absolute -top-px -right-px h-2.5 w-2.5 border-t border-r"
    style:border-color={rider.accent}
  ></div>
  <div
    class="pointer-events-none absolute -bottom-px -left-px h-2.5 w-2.5 border-b border-l opacity-50"
    style:border-color={rider.accent}
  ></div>
  <div
    class="pointer-events-none absolute -right-px -bottom-px h-2.5 w-2.5 border-b border-r opacity-50"
    style:border-color={rider.accent}
  ></div>

  <!-- Title bar — rider name + status badge -->
  <header
    class="flex items-center justify-between gap-3 border-b border-[var(--color-border)] bg-[var(--color-bg)]/60 px-4 py-2"
  >
    <h3
      class="title-bracket text-[calc(13px*var(--text-scale,1))] tracking-[0.04em] text-[var(--color-text)]"
    >
      {rider.name}
    </h3>
    <StatusBadge status={rider.status} />
  </header>

  <!-- Sandbox-missing ribbon. Takes priority over the first-launch
       hint because if the sandbox is gone, there's no first-launch to
       hint about — only a recovery to perform. -->
  {#if isMissing}
    <div
      class="flex items-start gap-2 border-b border-[var(--color-warn)]/40 bg-[var(--color-warn)]/10 px-4 py-2"
    >
      <span class="text-[var(--color-warn)] leading-tight">⚠</span>
      <p
        class="text-[calc(10px*var(--text-scale,1))] leading-[1.4] tracking-[0.04em] text-[var(--color-warn)]"
      >
        <span class="mono tracking-[0.18em] uppercase">Sandbox no longer exists.</span><br>
        The Sandboxie box this rider was using has been deleted outside of
        Bifrost. Use Remove to drop the orphaned record.
      </p>
    </div>
  {/if}

  <!-- First-launch hint ribbon -->
  {#if showFirstLaunchHint}
    <div
      class="flex items-start gap-2 border-b border-[var(--color-warn)]/40 bg-[var(--color-warn)]/10 px-4 py-2"
    >
      <span class="text-[var(--color-warn)] leading-tight">⚠</span>
      <p
        class="text-[calc(10px*var(--text-scale,1))] leading-[1.4] tracking-[0.04em] text-[var(--color-warn)]"
      >
        <span class="mono tracking-[0.18em] uppercase">The
        sandbox will inherit your default EVE Frontier launcher's account.</span><br>
        If you want clean start log out of the default launcher first.
      </p>
    </div>
  {/if}

  <!-- Portrait + accent picker overlay -->
  <div class="relative">
    <RiderPortrait
      name={rider.name}
      accent={rider.accent}
      active={isRunning}
      onEditAccent={() => (pickingAccent = !pickingAccent)}
    />

    {#if pickingAccent && palette.length > 0}
      <div
        class="absolute right-2 bottom-10 z-10 flex items-center gap-1.5 border border-[var(--color-border-hi)] bg-[var(--color-bg)]/95 p-1.5 backdrop-blur"
        role="dialog"
        aria-label="Pick accent colour"
      >
        {#each palette as color}
          <button
            class="h-5 w-5 cursor-pointer border transition-transform hover:scale-110 {color.toLowerCase() ===
            rider.accent.toLowerCase()
              ? 'border-[var(--color-focus)]'
              : 'border-[var(--color-border-hi)]'}"
            style:background={color}
            onclick={() => pickAccent(color)}
            title={color}
            aria-label="Set accent to {color}"
          ></button>
        {/each}
        <button
          class="mono ml-1 text-[calc(10px*var(--text-scale,1))] tracking-[0.15em] text-[var(--color-text-dim)] uppercase hover:text-[var(--color-text-muted)]"
          onclick={() => (pickingAccent = false)}
        >
          esc
        </button>
      </div>
    {/if}
  </div>

  <!-- "Energy" bar — purely cosmetic for now, hints that future state
       (wallet sync %, session readiness, etc.) will live here. -->
  <div class="h-1 bg-[var(--color-border)]">
    <div
      class="h-full transition-all"
      style:width={isRunning ? "100%" : "20%"}
      style:background={rider.accent}
      style:box-shadow={isRunning ? `0 0 12px ${rider.accent}` : "none"}
    ></div>
  </div>

  <!-- Stats grid — three big numbers, in-game telemetry pattern -->
  <div class="flex flex-col">
    {#if editingWallet}
      <div class="flex flex-col gap-2 px-4 py-3">
        <label
          class="text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-muted)] uppercase"
        >
          Wallet address
          <input
            type="text"
            placeholder="0x…"
            bind:value={walletInput}
            onkeydown={(e) => {
              if (e.key === "Enter") saveWallet();
              if (e.key === "Escape") cancelEditWallet();
            }}
            class="mono mt-2 w-full border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-[calc(11px*var(--text-scale,1))] normal-case tracking-normal text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
          />
        </label>
        <div class="flex gap-2 mt-1">
          <Button
            variant="primary"
            size="sm"
            disabled={savingWallet}
            onclick={saveWallet}
          >
            {savingWallet ? "Saving…" : "Save"}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={savingWallet}
            onclick={cancelEditWallet}
          >
            Cancel
          </Button>
        </div>
      </div>
    {:else}
      <!-- Stats grid: 3 columns, each [big value] / [tiny label] -->
      <div class="grid grid-cols-3 divide-x divide-[var(--color-border)]">
        <div class="flex flex-col items-start gap-0.5 px-3 py-2.5">
          <span class="mono text-[calc(15px*var(--text-scale,1))] leading-none text-[var(--color-text)]">
            {rider.eveBalance ?? "—"}
          </span>
          <span
            class="mono text-[calc(9px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            EVE
          </span>
        </div>
        <div class="flex flex-col items-start gap-0.5 px-3 py-2.5">
          <span
            class="mono text-[calc(15px*var(--text-scale,1))] leading-none text-[var(--color-text-muted)]"
          >
            {rider.walletBalance ?? "—"}
          </span>
          <span
            class="mono text-[calc(9px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            Gas
          </span>
        </div>
        <div class="flex flex-col items-start gap-0.5 px-3 py-2.5">
          <span
            class="mono truncate w-full text-[calc(12px*var(--text-scale,1))] leading-none text-[var(--color-text-muted)]"
            title={rider.sandbox}
          >
            {shortSandbox}
          </span>
          <span
            class="mono text-[calc(9px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            Box
          </span>
        </div>
      </div>

      <!-- Balance staleness. Tiny single-line stamp under the stats
           grid. Only renders when there IS a wallet to fetch and at
           least one successful fetch has happened — riders that
           never had a balance fetched stay unmarked rather than
           showing a confusing "never updated". Warn-yellow tint
           when stale (≥ 5 min). -->
      {#if balanceAge}
        <div
          class="border-t border-[var(--color-border)] px-4 py-1.5 text-[calc(9px*var(--text-scale,1))] tracking-[0.18em] uppercase {balanceAge.stale
            ? 'text-[var(--color-warn)]'
            : 'text-[var(--color-text-dim)]'}"
          title="Balances refresh every 30 s when the window is in the foreground and the Sui RPC isn't rate-limiting us."
        >
          <span class="mono">{balanceAge.label}</span>
        </div>
      {/if}

      <!-- Compact wallet line — just the address, self-evident `0x…` form -->
      <button
        class="flex items-center justify-between gap-2 border-t border-[var(--color-border)] px-4 py-2 text-left transition-colors hover:bg-[var(--color-bg)]/40"
        onclick={startEditWallet}
        title="Click to edit wallet address"
      >
        {#if rider.walletAddress}
          <span class="mono text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-muted)]"
            >{shortAddr}</span
          >
          <span
            class="mono text-[calc(9px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-dim)] uppercase"
            >Edit</span
          >
        {:else}
          <span class="mono text-[calc(11px*var(--text-scale,1))] text-[var(--color-accent)]"
            >+ Set wallet address</span
          >
          <span class="text-[var(--color-text-dim)]">›</span>
        {/if}
      </button>
    {/if}
  </div>

  <!-- Apps row — companion site shortcuts (lazy-loaded chunk).
       Renders only when wallet integration is actually ready
       (Brave + EVE Vault installed) AND there's at least one
       enabled site. The component itself is dynamically imported
       so users who haven't set up the wallet workflow never load
       the favicon-fetch code path. See the `$effect` in the
       script block. -->
  {#if AppsRowComponent && integrationReady() && configStore.enabledSites.length > 0}
    {@const Comp = AppsRowComponent}
    <Comp {rider} />
  {/if}

  <!-- Action footer.
       The primary CTA (Launch / Stop) is sized `lg` and `flex-1` so it
       dominates the row on narrow cards, but capped at `max-w-[260px]`
       so it doesn't sprawl across the full width of a card in the
       2- or 3-rider fixed layouts (where the card stretches to fit
       half / a third of the window). Excess footer space sits as
       calm gap between Launch and the Archive icon — feels less
       "button bar", more "primary action, quiet utility". Archive
       is a quiet icon-button on the RIGHT — less weight, hints at a
       non-destructive "stash" action (sandbox is preserved).
       Disabled while the rider is running. -->
  <footer
    class="flex items-center gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg)]/40 px-4 py-2.5"
  >
    {#if isMissing}
      <!-- Sandbox was deleted externally. Launch / Stop / Archive don't
           apply — there's nothing to act on. Only sensible action is to
           drop the orphan rider record. -->
      <Button
        variant="danger"
        size="lg"
        class="flex-1 max-w-[260px]"
        onclick={removeMissing}
      >
        Remove rider
      </Button>
    {:else if isRunning}
      <Button
        variant="danger"
        size="lg"
        class="flex-1 max-w-[260px]"
        disabled={syncing}
        onclick={() => riderStore.stop(rider.id)}
      >
        {syncing ? "Syncing…" : "Stop"}
      </Button>
    {:else}
      <Button
        variant="primary"
        size="lg"
        class="flex-1 max-w-[260px]"
        disabled={isBusy || syncing}
        onclick={() => riderStore.start(rider.id)}
      >
        ▶ {isBusy ? "Initialising…" : syncing ? "Syncing…" : "Launch"}
      </Button>
    {/if}
    {#if !isMissing}
      <button
        class="flex h-11 w-11 shrink-0 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)] disabled:cursor-not-allowed disabled:opacity-30"
        disabled={isRunning}
        onclick={() => riderStore.archive(rider.id)}
        aria-label="Archive rider {rider.name}"
        title={isRunning
          ? "Stop the rider before archiving"
          : "Stash this rider. Sandbox is preserved and can be restored later."}
      >
        <IconArchive size="18" />
      </button>
    {/if}
  </footer>
</article>
