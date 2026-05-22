<script lang="ts">
  import type { Pilot } from "../types";
  import { pilotStore } from "../stores/pilots.svelte";
  import Button from "./Button.svelte";

  interface Props {
    pilot: Pilot;
  }

  let { pilot }: Props = $props();

  let confirmingDelete = $state(false);

  const shortAddr = $derived(
    pilot.walletAddress
      ? `${pilot.walletAddress.slice(0, 6)}…${pilot.walletAddress.slice(-4)}`
      : "—",
  );
</script>

<article
  class="relative flex flex-col gap-4 border border-[var(--color-border)] bg-[var(--color-surface)]/20 p-5 opacity-70 transition-opacity hover:opacity-100"
>
  <div
    class="absolute top-0 right-0 left-0 h-[2px] opacity-30"
    style:background={pilot.accent}
  ></div>

  <header class="flex items-start justify-between gap-3">
    <div class="flex flex-col gap-1">
      <h3
        class="title-bracket text-[14px] text-[var(--color-text-muted)]"
      >
        {pilot.name}
      </h3>
      <div
        class="flex items-center gap-2 text-[10px] text-[var(--color-text-dim)] uppercase tracking-[0.2em]"
      >
        <span class="mono normal-case tracking-normal">{pilot.sandbox}</span>
        <span>›</span>
        <span>Archived</span>
      </div>
    </div>
  </header>

  {#if pilot.walletAddress}
    <div class="field">
      <span class="label">Wallet</span>
      <span class="leader"></span>
      <span class="value text-[var(--color-text-muted)]">{shortAddr}</span>
    </div>
  {/if}

  <footer class="flex flex-wrap gap-2">
    <Button variant="focus" onclick={() => pilotStore.restore(pilot.id)}>
      Restore
    </Button>
    {#if confirmingDelete}
      <Button
        variant="danger"
        onclick={() => pilotStore.deletePermanently(pilot.id)}
      >
        Confirm
      </Button>
      <Button variant="ghost" onclick={() => (confirmingDelete = false)}>
        Cancel
      </Button>
    {:else}
      <Button
        variant="ghost"
        onclick={() => (confirmingDelete = true)}
        title="Permanently remove the Bridge pilot record. The Sandboxie box itself is not deleted."
      >
        Delete…
      </Button>
    {/if}
  </footer>
</article>
