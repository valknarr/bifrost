<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Variant = "primary" | "ghost" | "danger" | "focus";
  type Size = "sm" | "md" | "lg";

  interface Props extends HTMLButtonAttributes {
    variant?: Variant;
    size?: Size;
    children: Snippet;
  }

  let {
    variant = "primary",
    size = "md",
    children,
    class: cls = "",
    ...rest
  }: Props = $props();

  const base =
    "inline-flex items-center justify-center gap-2 tracking-[0.2em] uppercase whitespace-nowrap font-medium transition-all duration-150 disabled:cursor-not-allowed disabled:opacity-30 disabled:[box-shadow:none] cursor-pointer relative";

  const sizes: Record<Size, string> = {
    sm: "px-3 py-1.5 text-[calc(10px*var(--text-scale,1))] min-h-[28px] min-w-[72px]",
    md: "px-5 py-2.5 text-[calc(11px*var(--text-scale,1))] min-h-[36px] min-w-[96px]",
    // lg is the "pilot card primary CTA" size — bigger hit-target +
    // beefier visual presence for Launch / Stop, where the user is
    // committing to a multi-second sandbox spin-up. Pairs with the
    // h-10/w-10 icon-button for Archive sitting next to it.
    lg: "px-6 py-3 text-[calc(12px*var(--text-scale,1))] min-h-[44px] min-w-[120px]",
  };

  // Primary + danger are *filled* by default so the commit-this-action
  // buttons pop against the dark UI without the user having to hover to
  // discover them. Hover bumps the glow + (for primary) lifts to the
  // accent-hi colour. Focus + ghost stay outline-only — they're for
  // secondary / configuration actions where ambient brightness would
  // compete with the primary CTA on the same row.
  const variants: Record<Variant, string> = {
    primary:
      "bg-[var(--color-accent)] border border-[var(--color-accent)] text-[var(--color-bg)] [box-shadow:0_0_10px_-3px_var(--color-accent)] hover:bg-[var(--color-accent-hi)] hover:border-[var(--color-accent-hi)] hover:[box-shadow:0_0_22px_-2px_var(--color-accent-hi)]",
    danger:
      "bg-[var(--color-danger)] border border-[var(--color-danger)] text-[var(--color-bg)] [box-shadow:0_0_10px_-3px_var(--color-danger)] hover:[box-shadow:0_0_22px_-2px_var(--color-danger)]",
    focus:
      "bg-transparent border border-[var(--color-focus)] text-[var(--color-focus)] hover:bg-[var(--color-focus)] hover:text-[var(--color-bg)] hover:[box-shadow:0_0_18px_-2px_var(--color-focus)]",
    ghost:
      "bg-transparent border border-[var(--color-border-hi)] text-[var(--color-text-muted)] hover:border-[var(--color-text-muted)] hover:text-[var(--color-text)]",
  };
</script>

<button class="{base} {sizes[size]} {variants[variant]} {cls}" {...rest}>
  {@render children()}
</button>
