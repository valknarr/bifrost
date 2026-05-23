<script lang="ts">
  // Discrete `v1.17.6` text styled as a link to the upstream project's
  // GitHub releases page. Used in the Settings detection / installer
  // panels so each version label is also a deep-link to its origin —
  // discoverability without an explicit "Manual ↗" button.

  import { openExternal } from "../external";
  import { vTag } from "../version";

  interface Props {
    version: string;
    /** Releases page URL — opens in the host browser on click. */
    url: string;
    /** Optional CSS class override for typography. */
    class?: string;
  }

  let { version, url, class: cls = "" }: Props = $props();

  // Normalised label. Upstream version strings are inconsistent:
  // Sandboxie stores "5.72.6" (no `v`), Brave's tag is "v1.90.124",
  // EVE Vault's tag is "v0.0.9". `vTag()` strips a leading `v` first
  // so the result is always exactly `v<digits.dots>` — without it the
  // Brave + EVE Vault rows rendered "vv1.90.124".
  const labelText = $derived(vTag(version));
</script>

<button
  type="button"
  class="mono cursor-pointer underline decoration-[var(--color-border-hi)] decoration-dotted underline-offset-2 transition-colors hover:decoration-[var(--color-focus)] hover:text-[var(--color-focus)] {cls}"
  onclick={() => openExternal(url)}
  title="View release notes on GitHub ({url})"
>
  {labelText}
</button>
