<script lang="ts">
  // Rider portrait. Today this is a shared placeholder image (pixel /
  // halftone style, matches the EVE-Frontier "data substrate" feel of
  // the rest of the UI). Per-rider identity is carried by the accent
  // colour — corner brackets, edge tick, and the "energy" bar in the
  // parent RiderCard. Once we wire up FusionAuth + portrait fetching
  // we'll swap `src` per rider; the layout stays the same.

  import riderPlaceholder from "$lib/assets/rider-placeholder.png";

  interface Props {
    name: string;
    accent: string;
    /** When true, intensifies the accent treatment for a running rider. */
    active?: boolean;
    /** Optional callback fired when the user clicks the pen icon to
     *  change the accent. When omitted the pen icon doesn't render. */
    onEditAccent?: () => void;
  }

  let { name, accent, active = false, onEditAccent }: Props = $props();
</script>

<div
  class="relative h-[240px] w-full overflow-hidden border-y border-[var(--color-border)] bg-[#03050a]"
>
  <!-- Portrait. object-position keeps the face in frame even though
       we're cropping a portrait-orientation source. The image is
       already sepia-toned to match the background palette, so we let
       its native colour show through (no desaturation); active state
       just bumps brightness/contrast slightly to signal the rider is
       live. -->
  <img
    src={riderPlaceholder}
    alt="Portrait of {name}"
    class="absolute inset-0 h-full w-full object-cover object-[center_18%] transition-all"
    style:filter={active
      ? "contrast(1.05) brightness(1.05)"
      : "contrast(1.0) brightness(0.85)"}
    draggable="false"
  />

  <!-- Vignette so the image blends into the surrounding panel rather
       than sitting on it as a hard rectangle. -->
  <div
    class="pointer-events-none absolute inset-0"
    style:background="radial-gradient(ellipse at center, transparent 65%, rgba(3,5,10,0.8) 100%)"
  ></div>

  <!-- Scanlines — same low-opacity pattern used elsewhere in the UI. -->
  <div
    class="absolute inset-0 pointer-events-none"
    style:background-image="repeating-linear-gradient(0deg, rgba(255,255,255,0.04) 0px, rgba(255,255,255,0.04) 1px, transparent 1px, transparent 3px)"
  ></div>

  <!-- Soft accent wash on the bottom edge so the colour still reads
       even without the monogram. Combined with the bar in the parent
       RiderCard, the rider's identity colour is unmistakable. -->
  <div
    class="pointer-events-none absolute right-0 bottom-0 left-0 h-6"
    style:background="linear-gradient(to top, {accent}{active
      ? '40'
      : '22'}, transparent)"
  ></div>

  <!-- Corner brackets — game-style measurement marks in the accent. -->
  <div
    class="pointer-events-none absolute top-2 left-2 h-3 w-3 border-t border-l"
    style:border-color={accent}
  ></div>
  <div
    class="pointer-events-none absolute top-2 right-2 h-3 w-3 border-t border-r"
    style:border-color={accent}
  ></div>
  <div
    class="pointer-events-none absolute bottom-2 left-2 h-3 w-3 border-b border-l"
    style:border-color={accent}
  ></div>
  <div
    class="pointer-events-none absolute right-2 bottom-2 h-3 w-3 border-b border-r"
    style:border-color={accent}
  ></div>

  <!-- Tick marks along edges (subtle measurement guides). -->
  <div
    class="pointer-events-none absolute top-1/2 left-0 h-2 w-1.5 -translate-y-1/2 bg-[var(--color-border-hi)]"
  ></div>
  <div
    class="pointer-events-none absolute top-1/2 right-0 h-2 w-1.5 -translate-y-1/2 bg-[var(--color-border-hi)]"
  ></div>

  <!-- Pen icon — change accent colour. Stays subtle until hovered. -->
  {#if onEditAccent}
    <button
      class="absolute right-2 bottom-2 flex h-6 w-6 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] bg-[var(--color-bg)]/70 text-[calc(11px*var(--text-scale,1))] text-[var(--color-text-muted)] opacity-50 transition-all hover:border-[var(--color-focus)] hover:text-[var(--color-focus)] hover:opacity-100"
      onclick={onEditAccent}
      title="Change accent colour"
      aria-label="Change accent colour"
    >
      ✎
    </button>
  {/if}
</div>
