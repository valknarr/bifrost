// Component test for UpdateBanner — the top-of-window strip that
// notifies the user a new signed release is ready. The component is
// store-driven (reads from `updaterStore`) so the test seeds the
// store's reactive state directly rather than passing props.
//
// Why this test exists: the v0.0.4/v0.0.5 regression where the
// `<UpdateBanner />` invocation was dropped from `Layout.svelte`
// would have been caught one layer higher (the Layout source-check
// test in `Layout.test.ts`), but this component test pins the
// banner's OWN contract — "given updaterStore.available is set,
// render the alert with the version label and the Restart/Later
// buttons" — so a refactor of the component internals can't
// silently regress the user-visible shape.

import { describe, expect, it, beforeEach } from "vitest";
import { render, screen } from "@testing-library/svelte";
import UpdateBanner from "./UpdateBanner.svelte";
import { updaterStore } from "../stores/updater.svelte";
import type { Update } from "@tauri-apps/plugin-updater";

// Minimal stub matching the small slice of `Update` the banner
// reads. `Update` from `@tauri-apps/plugin-updater` has many more
// fields (date, body, downloadAndInstall) we don't touch in render.
const fakeUpdate = (version: string): Update =>
  ({ version }) as unknown as Update;

describe("UpdateBanner", () => {
  beforeEach(() => {
    // Reset the store between tests so the previous test's
    // `available` doesn't leak into the next.
    updaterStore.available = null;
    updaterStore.installing = false;
    updaterStore.progress = null;
    updaterStore.error = null;
  });

  it("renders nothing when no update is available", () => {
    const { container } = render(UpdateBanner);
    // The outer `{#if available}` returns no DOM at all — the
    // container is empty (whitespace text node only).
    expect(container.querySelector("[role='alert']")).toBeNull();
  });

  it("renders the alert with the new version when available", () => {
    updaterStore.available = fakeUpdate("0.0.7");
    render(UpdateBanner);
    const alert = screen.getByRole("alert");
    expect(alert).toBeTruthy();
    expect(alert.textContent).toContain("0.0.7");
    expect(alert.textContent?.toLowerCase()).toContain("update available");
  });

  it("offers Restart and update + Later buttons in the idle-with-update state", () => {
    updaterStore.available = fakeUpdate("0.0.7");
    render(UpdateBanner);
    expect(
      screen.getByRole("button", { name: /restart and update/i }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: /later/i })).toBeTruthy();
  });

  it("hides the action buttons and shows progress while installing", () => {
    updaterStore.available = fakeUpdate("0.0.7");
    updaterStore.installing = true;
    updaterStore.progress = 42;
    render(UpdateBanner);
    // Progress meter is rendered (42% somewhere in the banner).
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("42");
    // Restart/Later buttons are gone while installing.
    expect(
      screen.queryByRole("button", { name: /restart and update/i }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: /^later$/i })).toBeNull();
  });

  it("swaps to Retry + Dismiss buttons when an install errors", () => {
    updaterStore.available = fakeUpdate("0.0.7");
    updaterStore.installing = false;
    updaterStore.error = "signature mismatch";
    render(UpdateBanner);
    expect(screen.getByRole("button", { name: /retry/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /dismiss/i })).toBeTruthy();
    // Error message is shown to the user.
    expect(screen.getByRole("alert").textContent).toContain(
      "signature mismatch",
    );
  });
});
