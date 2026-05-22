// Tiny wrapper around the Tauri shell plugin's `open()` so callers
// don't repeat the import dance. We only ever open vetted, hardcoded
// product URLs from Bridge's own UI (e.g. the official Sandboxie-Plus
// releases page) — never URLs sourced from untrusted content.
//
// `shell:allow-open` is granted in capabilities/default.json.

import { open as shellOpen } from "@tauri-apps/plugin-shell";

/** Canonical download / project URLs surfaced from the setup banner. */
export const externalLinks = {
  sandboxie: "https://github.com/sandboxie-plus/Sandboxie/releases/latest",
  eveFrontier: "https://www.evefrontier.com/en/download",
  brave: "https://github.com/brave/brave-browser/releases/latest",
  eveVault: "https://github.com/evefrontier/evevault/releases/latest",
} as const;

export async function openExternal(url: string): Promise<void> {
  try {
    await shellOpen(url);
  } catch (e) {
    // Swallow — opening a URL is best-effort; the UI already shows the
    // URL as text for fallback.
    console.warn("openExternal failed", url, e);
  }
}
