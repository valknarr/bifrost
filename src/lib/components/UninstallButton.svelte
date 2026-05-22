<script lang="ts">
  /**
   * Trash-icon button for "uninstall this managed component" actions
   * in the Settings panel. Wraps the consistent visual + the
   * `confirmDelete` guard prompt so the three installer rows
   * (Sandboxie, portable browser, EVE Vault) don't repeat the same
   * 20-line block 6× — once for the "update available" branch and
   * once for the "up to date" branch of each.
   *
   * The `component` string is used verbatim in both the confirm
   * prompt ("Remove {component} now?") and the aria-label / title
   * ("Uninstall {component}"). Picking a phrasing that reads
   * naturally in both contexts is the caller's job — "Sandboxie
   * Plus" works for proper-noun installs, "the portable browser"
   * works for generic components.
   */

  import { confirmDelete } from "../confirm";
  import IconDelete from "./IconDelete.svelte";

  interface Props {
    /** Subject of the action — shown in the confirm prompt and the
     *  aria-label. See file-level doc for phrasing guidance. */
    component: string;
    /** Optional one-liner context shown inside the confirm dialog,
     *  between the "Remove X now?" line and the "you can reinstall"
     *  footer. Used for high-stakes components (e.g. Sandboxie's
     *  kernel driver warning). */
    warning?: string;
    /** Disable the button while the upstream uninstall is in flight,
     *  so the user can't queue a second teardown. */
    busy?: boolean;
    /** Fired only after the user accepts the confirm prompt. */
    onConfirm: () => void;
  }

  let { component, warning, busy = false, onConfirm }: Props = $props();
</script>

<button
  class="flex h-7 w-7 cursor-pointer items-center justify-center border border-[var(--color-border-hi)] text-[var(--color-text-muted)] transition-colors hover:border-[var(--color-danger)] hover:text-[var(--color-danger)] disabled:cursor-not-allowed disabled:opacity-30"
  disabled={busy}
  onclick={() => {
    if (confirmDelete(component, warning)) onConfirm();
  }}
  aria-label="Uninstall {component}"
  title="Uninstall {component}"
>
  <IconDelete />
</button>
