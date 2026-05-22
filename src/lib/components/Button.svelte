<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Variant = "primary" | "ghost" | "danger" | "focus";
  type Size = "sm" | "md";

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
    sm: "px-3 py-1.5 text-[10px] min-h-[28px] min-w-[72px]",
    md: "px-5 py-2.5 text-[11px] min-h-[36px] min-w-[96px]",
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
