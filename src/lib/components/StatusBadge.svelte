<script lang="ts">
  import type { PilotStatus } from "../types";

  interface Props {
    status: PilotStatus;
  }

  let { status }: Props = $props();

  const styles: Record<
    PilotStatus,
    { dot: string; label: string; text: string; glow: string }
  > = {
    stopped: {
      dot: "bg-[var(--color-text-dim)]",
      label: "Standby",
      text: "text-[var(--color-text-muted)]",
      glow: "",
    },
    starting: {
      dot: "bg-[var(--color-warn)] animate-pulse",
      label: "Initialising",
      text: "text-[var(--color-warn)]",
      glow: "0 0 8px var(--color-warn)",
    },
    running: {
      dot: "bg-[var(--color-ok)]",
      label: "Online",
      text: "text-[var(--color-ok)]",
      glow: "0 0 8px var(--color-ok)",
    },
    error: {
      dot: "bg-[var(--color-danger)]",
      label: "Fault",
      text: "text-[var(--color-danger)]",
      glow: "0 0 8px var(--color-danger)",
    },
  };

  const s = $derived(styles[status]);
</script>

<span
  class="mono inline-flex items-center gap-2 text-[10px] tracking-[0.25em] uppercase {s.text}"
>
  <span class="h-1.5 w-1.5 {s.dot}" style:box-shadow={s.glow}></span>
  {s.label}
</span>
