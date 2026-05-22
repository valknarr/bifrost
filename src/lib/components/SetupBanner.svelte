<script lang="ts">
  import { onMount } from "svelte";
  import { statusStore } from "../stores/status.svelte";
  import { sandboxieStore } from "../stores/sandboxie.svelte";
  import { routeStore } from "../stores/route.svelte";
  import Button from "./Button.svelte";

  // Banner is shown above the pilot roster whenever a host dependency
  // is missing. We do NOT render the banner when status is still null
  // (initial probe in flight) to avoid a flash.
  const sandboxieOk = $derived(statusStore.status?.sandboxieInstalled ?? false);
  const frontierOk = $derived(statusStore.status?.frontierFound ?? false);
  const visible = $derived(statusStore.ready && statusStore.missingAny);

  // Refresh sandboxie state once on mount so the Settings tab's badge +
  // any cross-view derived UI stays accurate. The actual install action
  // lives on the Settings page — the banner only points there.
  onMount(() => {
    sandboxieStore.refresh();
  });

  interface MissingRow {
    label: string;
    detail: string;
  }

  // Only the missing components are rendered. Detected ones simply
  // disappear from the banner — nothing to do, nothing to look at.
  const missing = $derived<MissingRow[]>(
    [
      {
        ok: sandboxieOk,
        label: "Sandboxie",
        detail:
          "Required to run per-pilot game clients in isolation. Bridge can install Plus or Classic for you.",
      },
      {
        ok: frontierOk,
        label: "EVE Frontier client",
        detail:
          "The official game launcher. Install it once, then Bridge will run it inside each pilot's sandbox.",
      },
    ].filter((r) => !r.ok),
  );
</script>

{#if visible}
  <aside
    class="relative flex flex-col gap-3 border border-[var(--color-border-hi)] bg-[var(--color-surface)]/60 px-5 py-4"
  >
    <!-- Subtle corner brackets in the neutral accent colour — the
         urgency lives in the row + CTA, not the panel chrome. -->
    <span
      class="pointer-events-none absolute -top-px -left-px h-2.5 w-2.5 border-t border-l border-[var(--color-border-hi)]"
    ></span>
    <span
      class="pointer-events-none absolute -top-px -right-px h-2.5 w-2.5 border-t border-r border-[var(--color-border-hi)]"
    ></span>
    <span
      class="pointer-events-none absolute -bottom-px -left-px h-2.5 w-2.5 border-b border-l border-[var(--color-border-hi)]"
    ></span>
    <span
      class="pointer-events-none absolute -right-px -bottom-px h-2.5 w-2.5 border-b border-r border-[var(--color-border-hi)]"
    ></span>

    <header class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3">
        <span
          class="mono flex h-6 w-6 items-center justify-center border border-[var(--color-danger)] text-[12px] font-bold text-[var(--color-danger)]"
          style:box-shadow="0 0 8px -2px var(--color-danger)"
          aria-hidden="true"
        >
          !
        </span>
        <div class="flex flex-col leading-tight">
          <span
            class="mono text-[12px] tracking-[0.2em] text-[var(--color-text)] uppercase"
          >
            Setup required
          </span>
          <span
            class="text-[10px] tracking-[0.05em] text-[var(--color-text-muted)]"
          >
            Bridge can't launch pilots until {missing.length === 1
              ? "this component is"
              : "these components are"} installed.
          </span>
        </div>
      </div>
      <Button
        variant="danger"
        size="sm"
        onclick={() => routeStore.go("settings")}
      >
        Open Settings ›
      </Button>
    </header>

    <!-- Only missing components are listed; detected ones drop out of
         the banner entirely so the user's eye is drawn straight to what
         still needs attention. -->
    <ul
      class="flex flex-col gap-1.5 border-l border-[var(--color-danger)]/40 pl-3"
    >
      {#each missing as row}
        <li class="flex items-start gap-3">
          <span
            class="mt-1 h-1.5 w-1.5 shrink-0 bg-[var(--color-danger)]"
            style:box-shadow="0 0 6px var(--color-danger)"
          ></span>
          <div class="flex flex-col leading-tight">
            <span
              class="mono text-[11px] tracking-[0.18em] uppercase text-[var(--color-text)]"
            >
              {row.label}
              <span
                class="ml-2 text-[9px] tracking-[0.22em] text-[var(--color-danger)]"
              >
                ✕ Missing
              </span>
            </span>
            <span
              class="text-[10px] text-[var(--color-text-muted)] leading-snug"
            >
              {row.detail}
            </span>
          </div>
        </li>
      {/each}
    </ul>
  </aside>
{/if}
