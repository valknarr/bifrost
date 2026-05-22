<script lang="ts">
  // Discrete `v1.17.6` text styled as a link to the upstream project's
  // GitHub releases page. Used in the Settings detection / installer
  // panels so each version label is also a deep-link to its origin —
  // discoverability without an explicit "Manual ↗" button.

  import { openExternal } from "../external";

  interface Props {
    version: string;
    /** Releases page URL — opens in the host browser on click. */
    url: string;
    /** Optional CSS class override for typography. */
    class?: string;
  }

  let { version, url, class: cls = "" }: Props = $props();
</script>

<button
  type="button"
  class="mono cursor-pointer underline decoration-[var(--color-border-hi)] decoration-dotted underline-offset-2 transition-colors hover:decoration-[var(--color-focus)] hover:text-[var(--color-focus)] {cls}"
  onclick={() => openExternal(url)}
  title="View release notes on GitHub ({url})"
>
  v{version}
</button>
