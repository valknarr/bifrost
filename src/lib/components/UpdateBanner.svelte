<script lang="ts">
  // Auto-update banner. Renders only when `updaterStore.available`
  // is non-null, so for users who never have an update pending
  // this component is a no-op render. Sits at the top of the Layout
  // so it doesn't compete with the in-view content.
  //
  // Visual shape: thin accent-tinted bar with the new version
  // tag, "Update now" + "Later" buttons, and a progress meter
  // during install. Mirrors the existing `SetupBanner` pattern so
  // it reads as the same kind of "head-of-page advisory".
  import { updaterStore } from "../stores/updater.svelte";
  import Button from "./Button.svelte";

  // Derived view-state. `installing` short-circuits the action
  // buttons to a progress meter; `error` swaps the buttons for
  // a "Retry / Dismiss" pair.
  const available = $derived(updaterStore.available);
  const newVersion = $derived(updaterStore.newVersion);
  const installing = $derived(updaterStore.installing);
  const progress = $derived(updaterStore.progress);
  const error = $derived(updaterStore.error);
</script>

{#if available}
  <div
    class="mx-auto flex w-full max-w-6xl items-center gap-3 border-x border-b border-[var(--color-accent)]/40 bg-[var(--color-accent)]/8 px-4 py-2"
    role="alert"
    aria-live="polite"
  >
    <span
      class="mono text-[calc(10px*var(--text-scale,1))] tracking-[0.22em] text-[var(--color-accent)] uppercase"
    >
      ↑ Update available
    </span>
    <span class="text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-muted)]">
      Bifrost {newVersion} is ready to install.
    </span>

    {#if installing}
      <!-- Progress meter replaces the action buttons while the
           new installer downloads. Tauri runs makensis in `passive`
           mode after this so the user sees a small Windows progress
           dialog AFTER our progress bar finishes. -->
      <div class="ml-auto flex flex-1 max-w-[200px] items-center gap-2">
        <div class="relative h-[2px] flex-1 overflow-hidden bg-[var(--color-border)]">
          <div
            class="absolute top-0 left-0 h-full transition-all"
            style:width="{progress ?? 0}%"
            style:background="var(--color-accent)"
          ></div>
        </div>
        <span class="mono text-[calc(10px*var(--text-scale,1))] text-[var(--color-text-muted)]">
          {progress ?? 0}%
        </span>
      </div>
    {:else if error}
      <span
        class="ml-auto text-[calc(10px*var(--text-scale,1))] text-[var(--color-danger)]"
      >
        {error}
      </span>
      <Button size="sm" variant="ghost" onclick={() => updaterStore.dismiss()}>
        Dismiss
      </Button>
      <Button size="sm" variant="focus" onclick={() => updaterStore.installAndRelaunch()}>
        Retry
      </Button>
    {:else}
      <div class="ml-auto flex items-center gap-2">
        <Button size="sm" variant="ghost" onclick={() => updaterStore.dismiss()}>
          Later
        </Button>
        <Button
          size="sm"
          variant="primary"
          onclick={() => updaterStore.installAndRelaunch()}
        >
          Restart and update
        </Button>
      </div>
    {/if}
  </div>
{/if}
