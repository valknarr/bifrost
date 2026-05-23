<script lang="ts">
  import type { Rider } from "../types";
  import { riderStore } from "../stores/riders.svelte";
  import Button from "./Button.svelte";

  interface Props {
    rider: Rider;
  }

  let { rider }: Props = $props();

  let confirmingDelete = $state(false);

  const shortAddr = $derived(
    rider.walletAddress
      ? `${rider.walletAddress.slice(0, 6)}…${rider.walletAddress.slice(-4)}`
      : "—",
  );
</script>

<article
  class="relative flex flex-col gap-4 border border-[var(--color-border)] bg-[var(--color-surface)]/20 p-5 opacity-70 transition-opacity hover:opacity-100"
>
  <div
    class="absolute top-0 right-0 left-0 h-[2px] opacity-30"
    style:background={rider.accent}
  ></div>

  <header class="flex items-start justify-between gap-3">
    <div class="flex flex-col gap-1">
      <h3
        class="title-bracket text-[calc(14px*var(--text-scale,1))] text-[var(--color-text-muted)]"
      >
        {rider.name}
      </h3>
      <div
        class="flex items-center gap-2 text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-dim)] uppercase tracking-[0.2em]"
      >
        <span class="mono normal-case tracking-normal">{rider.sandbox}</span>
        <span>›</span>
        <span>Archived</span>
      </div>
    </div>
  </header>

  {#if rider.walletAddress}
    <div class="field">
      <span class="label">Wallet</span>
      <span class="leader"></span>
      <span class="value text-[var(--color-text-muted)]">{shortAddr}</span>
    </div>
  {/if}

  <!-- All buttons at `size="sm"` so the three-button delete-confirm
       state (Restore / Confirm / Cancel) fits on one row inside the
       ~280 px card content width without wrapping. `md` buttons are
       `min-w-[96px]` each, which forces the third button to a new
       row and causes a visible layout jump when toggling between the
       two states; `sm` is `min-w-[72px]` and clears that comfortably.
       `flex-wrap` stays as a defensive fallback for very narrow
       rendering (e.g. user zoom or future layout shifts). -->
  <footer class="flex flex-wrap gap-2">
    <Button size="sm" variant="focus" onclick={() => riderStore.restore(rider.id)}>
      Restore
    </Button>
    {#if confirmingDelete}
      <Button
        size="sm"
        variant="danger"
        onclick={() => riderStore.deletePermanently(rider.id)}
      >
        Confirm
      </Button>
      <Button size="sm" variant="ghost" onclick={() => (confirmingDelete = false)}>
        Cancel
      </Button>
    {:else}
      <Button
        size="sm"
        variant="ghost"
        onclick={() => (confirmingDelete = true)}
        title="Permanently remove the rider AND its Sandboxie box + browser profile. This cannot be undone."
      >
        Delete…
      </Button>
    {/if}
  </footer>
</article>
