<script lang="ts">
  // Minimalist trash-can glyph used by the Settings detection panel for
  // each "uninstall this component" button. Single-stroke, sized via
  // currentColor + em units so it inherits the button's text colour and
  // scales with font size.

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
  <!-- Lid -->
  <path d="M2 4h12" />
  <path d="M6 4V2.5h4V4" />
  <!-- Bin -->
  <path d="M3.5 4l.75 9.5h7.5L12.5 4" />
  <!-- Inner ribs -->
  <path d="M6.5 6.5v5" />
  <path d="M9.5 6.5v5" />
</svg>
