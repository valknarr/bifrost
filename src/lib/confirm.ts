// Centralised confirm dialog for destructive actions.
//
// Tauri's webview ships with a working `window.confirm()`. Routing every
// "are you sure?" prompt through this helper means:
//   - The message format ("Remove X now? … You can reinstall it later.")
//     is consistent across SettingsView (component uninstalls) and any
//     other view that needs a destructive guard.
//   - When we eventually swap the native dialog for a styled in-app
//     modal, there's only one call site to migrate.

/**
 * Ask the user to confirm tearing down `component` (e.g. "Sandboxie Plus",
 * "the EVE Vault extension"). Returns true if they accepted.
 *
 * `warning` is an optional one-liner injected between the prompt and the
 * "you can reinstall" line — used for high-stakes components like
 * Sandboxie where the kernel driver is involved.
 */
export function confirmDelete(component: string, warning?: string): boolean {
  const lines = [
    `Remove ${component} now?`,
    warning ?? "",
    "You can reinstall it again at any time from this panel.",
  ].filter(Boolean);
  return window.confirm(lines.join("\n\n"));
}

/**
 * Generic confirm prompt for destructive actions that don't fit the
 * "remove X now? you can reinstall" template — e.g. removing an
 * orphaned pilot whose sandbox is already gone, where reinstalling
 * isn't a meaningful answer. Same migration story as `confirmDelete`:
 * when the in-app modal lands, both wrappers swap together so
 * call-sites don't need to change.
 */
export function confirmDestructive(message: string): boolean {
  return window.confirm(message);
}
