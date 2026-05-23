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

/**
 * Inner-window widths (in CSS pixels) that match each roster-column
 * preset comfortably. Computed once and shared so the "2 pilots"
 * choice in Settings always snaps to the same width across launches.
 *
 * Math:
 *   - Each pilot card targets ~320 px content width
 *   - 16 px grid gap between cards
 *   - 48 px horizontal padding inside the section (`px-6`)
 *   - ~24 px slack for scrollbar + window-chrome variance
 *
 * 2 cards × 320 + 16 + 48 + 24 = 728 → round to 800 for breathing room
 * 3 cards × 320 + 32 + 48 + 24 = 1064 → round to 1120
 *
 * "Auto" deliberately has no entry — picking Auto leaves the window
 * size alone so a user who carefully sized things isn't reset on
 * pick.
 */
export const ROSTER_WINDOW_WIDTHS: Record<2 | 3, number> = {
  2: 800,
  3: 1120,
};

/**
 * Resize the window to match the chosen roster-column preset.
 * No-ops for `0` (auto) — picking Auto means "stop telling me what
 * the window width should be", not "reset to default".
 *
 * Keeps the current window height intact: the user may have grown
 * the window vertically to see more pilots and we'd rather not undo
 * that on a horizontal-layout pick. Failures bubble up so the
 * caller can surface them in the settings error banner.
 */
export async function applyRosterWindowSize(columns: number): Promise<void> {
  if (columns !== 2 && columns !== 3) return;
  const targetWidth = ROSTER_WINDOW_WIDTHS[columns];
  const { getCurrentWindow, LogicalSize } = await import(
    "@tauri-apps/api/window"
  );
  const win = getCurrentWindow();
  // Read the current inner size so we can preserve height. Tauri
  // returns physical pixels; convert via the scale factor so we
  // hand `setSize` the same logical units we used for the width.
  const scale = await win.scaleFactor();
  const inner = await win.innerSize();
  const currentHeightLogical = inner.height / scale;
  await win.setSize(new LogicalSize(targetWidth, currentHeightLogical));
}

/**
 * Read the live inner-window size, converted to logical CSS pixels.
 * Used both by the debounced resize listener (to know what to save)
 * and by callers that want to capture the current geometry directly.
 */
export async function currentWindowSize(): Promise<{
  width: number;
  height: number;
}> {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  const scale = await win.scaleFactor();
  const inner = await win.innerSize();
  return {
    width: Math.round(inner.width / scale),
    height: Math.round(inner.height / scale),
  };
}

/**
 * Set the window to a previously-saved Auto-mode size, in logical
 * pixels. Wraps `LogicalSize` so callers don't need to know about
 * the Tauri-side units. No-ops on null/invalid input so the typical
 * "load config, hand its optional fields to this function" path
 * stays one-line at the call site.
 */
export async function applySavedWindowSize(
  width: number | null,
  height: number | null,
): Promise<void> {
  if (
    width == null ||
    height == null ||
    !Number.isFinite(width) ||
    !Number.isFinite(height)
  ) {
    return;
  }
  const { getCurrentWindow, LogicalSize } = await import(
    "@tauri-apps/api/window"
  );
  await getCurrentWindow().setSize(new LogicalSize(width, height));
}

/**
 * Subscribe to window-resize events with a trailing-edge debounce.
 * The supplied `onStable` callback fires `delayMs` after the user
 * stops resizing — once, with the final size — not on every pixel
 * of a drag. Returns an unsubscribe function the caller can hold
 * onto for teardown.
 *
 * Why debounce in the listener (not at the call site): every
 * caller would otherwise duplicate the same timer logic. Centralising
 * here also means we keep the Tauri-side event listener attached
 * once instead of attaching + detaching per call.
 *
 * The Tauri event listener returns a `Promise<UnlistenFn>`; the
 * unsubscribe returned by this function awaits it lazily so callers
 * who tear down before the listener is fully attached still detach
 * correctly when the registration resolves.
 */
export function onWindowResizeStable(
  delayMs: number,
  onStable: (size: { width: number; height: number }) => void,
): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let detach: (() => void) | null = null;
  let cancelled = false;

  (async () => {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const unlisten = await getCurrentWindow().onResized(() => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(async () => {
        timer = null;
        try {
          const size = await currentWindowSize();
          onStable(size);
        } catch (e) {
          // Listener errors are non-fatal; log so a regression that
          // breaks scaleFactor/innerSize doesn't pass silently.
          console.warn("onWindowResizeStable: read size failed", e);
        }
      }, delayMs);
    });
    if (cancelled) {
      unlisten();
    } else {
      detach = unlisten;
    }
  })().catch((e) => {
    console.warn("onWindowResizeStable: attach failed", e);
  });

  return () => {
    cancelled = true;
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    if (detach) {
      detach();
      detach = null;
    }
  };
}
