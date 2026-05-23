// Thin wrapper around Tauri's webview zoom so the rest of the code
// doesn't have to know about `getCurrentWebview()`. Two callers:
//
//   1. App.svelte on mount — applies the persisted zoom before the
//      user sees the UI.
//   2. SettingsView when a preset is clicked — pairs with
//      `api.setUiZoom()` so the change is visible immediately AND
//      saved to disk for next launch.
//
// We import lazily to keep the cold path off the main bundle: the
// webview module is only needed when zoom actually changes.

/**
 * Three canonical presets surfaced in the Settings UI. Kept as a
 * union of string keys (vs. just floats) so the picker can show a
 * label without a separate lookup.
 */
export const ZOOM_PRESETS = {
  compact: { value: 0.9, label: "Compact", description: "Denser layout, more info per screen" },
  default: { value: 1.0, label: "Default", description: "Bifrost's reference design" },
  comfortable: { value: 1.15, label: "Comfortable", description: "Bigger text, easier on the eyes" },
} as const;

export type ZoomPreset = keyof typeof ZOOM_PRESETS;

/** Match a raw zoom float back to its preset name, or "custom" if it
 *  doesn't line up with any of the three. Used by the picker to
 *  highlight the active preset. We compare with a small epsilon so
 *  round-trips through JSON / setZoom don't drift the selection. */
export function presetFor(zoom: number): ZoomPreset | "custom" {
  for (const [key, p] of Object.entries(ZOOM_PRESETS) as [ZoomPreset, typeof ZOOM_PRESETS[ZoomPreset]][]) {
    if (Math.abs(zoom - p.value) < 0.001) return key;
  }
  return "custom";
}

/**
 * Apply a zoom factor to the running webview. Wraps the dynamic
 * import so callers don't need to know about `@tauri-apps/api`.
 *
 * Throws on failure — historically this swallowed errors silently,
 * which masked a missing-permission regression for an entire commit
 * cycle. Callers who genuinely want best-effort behaviour (e.g.
 * App.svelte's startup application of the persisted value, where a
 * silent fallback to 1.0 is acceptable) should wrap in their own
 * try/catch. The Settings picker re-throws so the user sees the
 * error in the panel.
 */
export async function applyZoom(zoom: number): Promise<void> {
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  await getCurrentWebview().setZoom(zoom);
}
