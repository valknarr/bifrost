<script lang="ts">
  import { onMount } from "svelte";
  import type { Pilot } from "../types";
  import { pilotStore } from "../stores/pilots.svelte";
  import { integrationReady } from "../stores/vault.svelte";
  import { configStore } from "../stores/config.svelte";
  import { api } from "../tauri";
  import { formatBackendError } from "../error";
  import Button from "./Button.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import PilotPortrait from "./PilotPortrait.svelte";

  interface Props {
    pilot: Pilot;
  }

  let { pilot }: Props = $props();

  let openingBrowser = $state(false);
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
    await pilotStore.setAccent(pilot.id, color);
    pickingAccent = false;
  }

  async function openBrowser() {
    openingBrowser = true;
    try {
      await api.openPilotBrowser(pilot.id);
    } catch (e) {
      pilotStore.error = formatBackendError(e);
    } finally {
      openingBrowser = false;
    }
  }

  async function openApp(url: string) {
    try {
      await api.openPilotApp(pilot.id, url);
    } catch (e) {
      pilotStore.error = formatBackendError(e);
    }
  }

  const shortAddr = $derived(
    pilot.walletAddress
      ? `${pilot.walletAddress.slice(0, 6)}…${pilot.walletAddress.slice(-4)}`
      : "—",
  );

  // Inline wallet editor state
  let editingWallet = $state(false);
  let walletInput = $state("");
  let savingWallet = $state(false);

  function startEditWallet() {
    walletInput = pilot.walletAddress ?? "";
    editingWallet = true;
  }

  async function saveWallet() {
    savingWallet = true;
    try {
      await pilotStore.setWallet(pilot.id, walletInput.trim());
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
    pilot.sandbox.length > 18
      ? `${pilot.sandbox.slice(0, 8)}…${pilot.sandbox.slice(-4)}`
      : pilot.sandbox,
  );

  const isRunning = $derived(pilot.status === "running");
  const isBusy = $derived(pilot.status === "starting");
  const showFirstLaunchHint = $derived(
    !pilot.launchedAtLeastOnce && !isRunning,
  );
</script>

<article
  class="group relative flex flex-col bg-[var(--color-surface)]/60 transition-colors {isRunning
    ? 'border border-[var(--color-focus)]/60'
    : 'border border-[var(--color-border)] hover:border-[var(--color-focus)]/60'}"
  style:--pilot-accent={pilot.accent}
>
  <!-- Outer corner brackets (in-game window-corner motif) -->
  <div
    class="pointer-events-none absolute -top-px -left-px h-2.5 w-2.5 border-t border-l"
    style:border-color={pilot.accent}
  ></div>
  <div
    class="pointer-events-none absolute -top-px -right-px h-2.5 w-2.5 border-t border-r"
    style:border-color={pilot.accent}
  ></div>
  <div
    class="pointer-events-none absolute -bottom-px -left-px h-2.5 w-2.5 border-b border-l opacity-50"
    style:border-color={pilot.accent}
  ></div>
  <div
    class="pointer-events-none absolute -right-px -bottom-px h-2.5 w-2.5 border-b border-r opacity-50"
    style:border-color={pilot.accent}
  ></div>

  <!-- Title bar — pilot name + status badge -->
  <header
    class="flex items-center justify-between gap-3 border-b border-[var(--color-border)] bg-[var(--color-bg)]/60 px-4 py-2"
  >
    <h3
      class="title-bracket text-[13px] tracking-[0.04em] text-[var(--color-text)]"
    >
      {pilot.name}
    </h3>
    <StatusBadge status={pilot.status} />
  </header>

  <!-- First-launch hint ribbon -->
  {#if showFirstLaunchHint}
    <div
      class="flex items-start gap-2 border-b border-[var(--color-warn)]/40 bg-[var(--color-warn)]/10 px-4 py-2"
    >
      <span class="text-[var(--color-warn)] leading-tight">⚠</span>
      <p
        class="text-[10px] leading-[1.4] tracking-[0.04em] text-[var(--color-warn)]"
      >
        <span class="mono tracking-[0.18em] uppercase">First launch</span> · the
        sandbox will inherit your default EVE Frontier launcher's account.
        Log out of the default launcher first, or log out + sign in as this
        pilot inside the sandbox after launch.
      </p>
    </div>
  {/if}

  <!-- Portrait + accent picker overlay -->
  <div class="relative">
    <PilotPortrait
      name={pilot.name}
      accent={pilot.accent}
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
            pilot.accent.toLowerCase()
              ? 'border-[var(--color-focus)]'
              : 'border-[var(--color-border-hi)]'}"
            style:background={color}
            onclick={() => pickAccent(color)}
            title={color}
            aria-label="Set accent to {color}"
          ></button>
        {/each}
        <button
          class="mono ml-1 text-[10px] tracking-[0.15em] text-[var(--color-text-dim)] uppercase hover:text-[var(--color-text-muted)]"
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
      style:background={pilot.accent}
      style:box-shadow={isRunning ? `0 0 12px ${pilot.accent}` : "none"}
    ></div>
  </div>

  <!-- Stats grid — three big numbers, in-game telemetry pattern -->
  <div class="flex flex-col">
    {#if editingWallet}
      <div class="flex flex-col gap-2 px-4 py-3">
        <label
          class="text-[10px] tracking-[0.2em] text-[var(--color-text-muted)] uppercase"
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
            class="mono mt-2 w-full border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-[11px] normal-case tracking-normal text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
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
          <span class="mono text-[15px] leading-none text-[var(--color-text)]">
            {pilot.eveBalance ?? "—"}
          </span>
          <span
            class="mono text-[9px] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            EVE
          </span>
        </div>
        <div class="flex flex-col items-start gap-0.5 px-3 py-2.5">
          <span
            class="mono text-[15px] leading-none text-[var(--color-text-muted)]"
          >
            {pilot.walletBalance ?? "—"}
          </span>
          <span
            class="mono text-[9px] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            Gas
          </span>
        </div>
        <div class="flex flex-col items-start gap-0.5 px-3 py-2.5">
          <span
            class="mono truncate w-full text-[12px] leading-none text-[var(--color-text-muted)]"
            title={pilot.sandbox}
          >
            {shortSandbox}
          </span>
          <span
            class="mono text-[9px] tracking-[0.22em] text-[var(--color-text-dim)] uppercase"
          >
            Box
          </span>
        </div>
      </div>

      <!-- Compact wallet line — just the address, self-evident `0x…` form -->
      <button
        class="flex items-center justify-between gap-2 border-t border-[var(--color-border)] px-4 py-2 text-left transition-colors hover:bg-[var(--color-bg)]/40"
        onclick={startEditWallet}
        title="Click to edit wallet address"
      >
        {#if pilot.walletAddress}
          <span class="mono text-[11px] text-[var(--color-text-muted)]"
            >{shortAddr}</span
          >
          <span
            class="mono text-[9px] tracking-[0.2em] text-[var(--color-text-dim)] uppercase"
            >Edit</span
          >
        {:else}
          <span class="mono text-[11px] text-[var(--color-accent)]"
            >+ Set wallet address</span
          >
          <span class="text-[var(--color-text-dim)]">›</span>
        {/if}
      </button>
    {/if}
  </div>

  <!-- Apps row — only renders when wallet integration is ready -->
  {#if integrationReady()}
    <div
      class="flex items-center justify-between gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg)]/20 px-4 py-1.5"
    >
      <div class="flex flex-wrap items-center gap-1.5">
        {#each configStore.enabledSites as site (site.url)}
          <button
            class="mono group flex h-7 w-7 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] bg-transparent text-[10px] font-bold text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)]"
            onclick={() => openApp(site.url)}
            title="{site.name} — {site.url}"
            aria-label="Open {site.name} as {pilot.name}"
          >
            {site.icon}
          </button>
        {/each}
        <!-- Visual separator + Configure (opens EVE Vault popup) -->
        {#if configStore.sites.length > 0}
          <span
            class="mx-1 h-5 w-px bg-[var(--color-border-hi)]"
            aria-hidden="true"
          ></span>
        {/if}
        <button
          class="mono group flex h-7 w-7 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] bg-transparent text-[12px] text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-focus)] hover:text-[var(--color-focus)] disabled:opacity-40 disabled:cursor-not-allowed"
          disabled={openingBrowser}
          onclick={openBrowser}
          title="Configure · open EVE Vault for this pilot"
          aria-label="Open EVE Vault wallet for {pilot.name}"
        >
          ⚙
        </button>
      </div>
    </div>
  {/if}

  <!-- Action footer -->
  <footer
    class="flex items-center gap-2 border-t border-[var(--color-border)] bg-[var(--color-bg)]/40 px-4 py-2.5"
  >
    {#if isRunning}
      <Button
        variant="danger"
        class="flex-1"
        onclick={() => pilotStore.stop(pilot.id)}
      >
        Stop
      </Button>
      <Button variant="ghost" disabled>Archive</Button>
    {:else}
      <Button
        variant="primary"
        class="flex-1"
        disabled={isBusy}
        onclick={() => pilotStore.start(pilot.id)}
      >
        ▶ {isBusy ? "Initialising…" : "Launch"}
      </Button>
      <Button
        variant="ghost"
        onclick={() => pilotStore.archive(pilot.id)}
        title="Stash this pilot. Sandbox is preserved and can be restored later."
      >
        Archive
      </Button>
    {/if}
  </footer>
</article>
