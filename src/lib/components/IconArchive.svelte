<script lang="ts">
  // Minimalist archive-box glyph used by the PilotCard footer for the
  // "stash this pilot, sandbox preserved" action. Mirrors IconDelete's
  // single-stroke / currentColor / 16-unit viewBox conventions so the
  // two read as a family.

  interface Props {
    /** Pixel size override; defaults to 1em for inline use. */
    size?: number | string;
    class?: string;
  }

  let { size = "1em", class: cls = "" }: Props = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="square"
  width={size}
  height={size}
  class={cls}
  aria-hidden="true"
>
  <!-- Lid — a wider rectangle sitting on top of the box body. -->
  <rect x="1.75" y="2.5" width="12.5" height="3" />
  <!-- Box body — open rectangle below the lid. -->
  <path d="M3 5.5v8h10v-8" />
  <!-- Pull notch / handle line — hints at "open this box". -->
  <path d="M6.5 9h3" />
</svg>
