<script lang="ts">
  import type { DiscoveredBox } from "../types";
  import { riderStore } from "../stores/riders.svelte";
  import { confirmDelete } from "../confirm";
  import Button from "./Button.svelte";
  import IconDelete from "./IconDelete.svelte";

  interface Props {
    box: DiscoveredBox;
  }

  let { box }: Props = $props();

  // Seed the editor from the box name once, then let the user edit it
  // freely. The Svelte warning is acknowledged: we deliberately don't
  // re-derive from `box.name` (would clobber user input).
  // svelte-ignore state_referenced_locally
  let displayName = $state(box.name);
  let adopting = $state(false);
  let deleting = $state(false);

  async function handleAdopt() {
    if (!displayName.trim()) return;
    adopting = true;
    try {
      await riderStore.adopt(box.name, displayName.trim());
    } finally {
      adopting = false;
    }
  }

  async function handleDelete() {
    const ok = confirmDelete(
      `sandbox "${box.name}"`,
      `This terminates any processes running inside the box and wipes ` +
        `C:\\Sandbox\\<user>\\${box.name}\\ from disk. The box itself is ` +
        `removed from Sandboxie.ini.`,
    );
    if (!ok) return;
    deleting = true;
    try {
      await riderStore.deleteSandbox(box.name);
    } finally {
      deleting = false;
    }
  }
</script>

<article
  class="relative flex flex-col gap-4 border border-dashed border-[var(--color-border)] bg-[var(--color-surface)]/30 p-5 transition-colors hover:border-[var(--color-border-hi)]"
>
  <div
    class="pointer-events-none absolute -top-px -left-px h-2 w-2 border-t border-l border-[var(--color-text-dim)]"
  ></div>
  <div
    class="pointer-events-none absolute -top-px -right-px h-2 w-2 border-t border-r border-[var(--color-text-dim)]"
  ></div>

  <header class="flex flex-col gap-1">
    <h3 class="title-bracket mono text-[calc(14px*var(--text-scale,1))] text-[var(--color-text-muted)]">
      {box.name}
    </h3>
    <p class="text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-dim)] uppercase">
      Unmanaged sandbox
    </p>
  </header>

  <div class="flex flex-col gap-2">
    <label class="flex flex-col gap-2 text-[calc(10px*var(--text-scale,1))] tracking-[0.2em] text-[var(--color-text-muted)] uppercase">
      Rider name
      <input
        type="text"
        placeholder="Designation…"
        bind:value={displayName}
        onkeydown={(e) => e.key === "Enter" && handleAdopt()}
        class="mono border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm normal-case tracking-normal text-[var(--color-text)] placeholder:text-[var(--color-text-dim)] focus:border-[var(--color-focus)] focus:outline-none"
      />
    </label>
    <!-- Adopt is the primary commit action; the trash sits beside it
         for users who know a discovered leftover isn't worth adopting. -->
    <div class="flex items-stretch gap-2">
      <Button
        class="flex-1"
        onclick={handleAdopt}
        disabled={adopting || deleting || !displayName.trim()}
      >
        {adopting ? "Adopting…" : "Adopt as rider"}
      </Button>
      <button
        class="flex w-9 shrink-0 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-danger)] hover:text-[var(--color-danger)] disabled:cursor-not-allowed disabled:opacity-30"
        disabled={adopting || deleting}
        onclick={handleDelete}
        aria-label="Delete this sandbox"
        title="Delete this sandbox"
      >
        <IconDelete />
      </button>
    </div>
  </div>
</article>
