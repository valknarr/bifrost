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

  /** Normalised label. Upstream version strings are inconsistent:
   *  - Sandboxie's installer marker stores "5.72.6" (no `v`)
   *  - Brave's GitHub release tag is "v1.90.124" (with `v`)
   *  - EVE Vault's release tag is "v0.0.9" (with `v`)
   *  Without normalisation we got `vv1.90.124` for the Brave +
   *  EVE Vault rows in Settings (the component's "v" prefix
   *  doubling with the tag's). Strip a leading `v` first so the
   *  result is always exactly `v<digits.dots>`. */
  const labelText = $derived.by(() => {
    const stripped = version.startsWith("v") ? version.slice(1) : version;
    return `v${stripped}`;
  });
</script>

<button
  type="button"
  class="mono cursor-pointer underline decoration-[var(--color-border-hi)] decoration-dotted underline-offset-2 transition-colors hover:decoration-[var(--color-focus)] hover:text-[var(--color-focus)] {cls}"
  onclick={() => openExternal(url)}
  title="View release notes on GitHub ({url})"
>
  {labelText}
</button>
