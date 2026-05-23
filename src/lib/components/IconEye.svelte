<script lang="ts">
  // Show/hide toggle used by the companion-sites list. `hidden=false`
  // renders an open eye (site is visible to riders); `hidden=true`
  // renders a struck-through eye (site is hidden / disabled). Single
  // stroke, sized via currentColor + em so it tracks the host font.

  interface Props {
    /** When true, draw the struck-through "hidden" variant. */
    hidden?: boolean;
    /** Pixel size override; defaults to 1em for inline use. */
    size?: number | string;
    class?: string;
  }

  let { hidden = false, size = "1em", class: cls = "" }: Props = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="round"
  stroke-linejoin="round"
  width={size}
  height={size}
  class={cls}
  aria-hidden="true"
>
  <!-- Almond outline -->
  <path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" />
  <!-- Pupil -->
  <circle cx="8" cy="8" r="1.8" />
  {#if hidden}
    <!-- Strike-through bar for hidden state -->
    <path d="M2.5 13.5 13.5 2.5" />
  {/if}
</svg>
