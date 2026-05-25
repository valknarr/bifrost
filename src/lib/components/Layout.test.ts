// Layout-level regression guard.
//
// v0.0.4 and v0.0.5 shipped with the `<UpdateBanner />` element
// accidentally removed from Layout.svelte's markup during a grid
// rearrangement — the `import UpdateBanner from "./UpdateBanner.svelte"`
// statement and the explanatory comment block both stayed in place,
// but the actual invocation didn't. Users on those releases could
// detect updates (`updaterStore.check()` ran fine, Settings → About
// reported the new version) but had no banner element in the DOM
// to click, breaking the one-click in-app update path.
//
// A full component render of Layout would be the prettier test, but
// Layout takes a Snippet `children` prop (Svelte 5) and drags in
// several stores; a focused source-text assertion is both cheaper
// AND catches the EXACT regression class we hit, without depending
// on store-mocking gymnastics. The trade-off is honest: this test
// pins the markup-level invariant "Layout invokes UpdateBanner",
// not "the rendered banner behaves correctly" — that part is
// covered by `UpdateBanner.test.ts` next to this file.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Vitest runs with `process.cwd()` at the project root, so resolve
// from there. `import.meta.url` would be the natural choice but
// jsdom replaces it with a non-`file://` URL that
// `fileURLToPath()` rejects.
const layoutSrc = readFileSync(
  join(process.cwd(), "src", "lib", "components", "Layout.svelte"),
  "utf-8",
);

describe("Layout.svelte — UpdateBanner wiring", () => {
  it("imports the UpdateBanner component", () => {
    // Belt-and-braces: import without invocation is exactly what
    // the v0.0.4/v0.0.5 regression looked like (only the import
    // survived). We assert both halves so a future cleanup that
    // accidentally drops EITHER half trips the test.
    expect(layoutSrc).toMatch(
      /import\s+UpdateBanner\s+from\s+["']\.\/UpdateBanner\.svelte["']/,
    );
  });

  it("invokes <UpdateBanner /> somewhere in the markup", () => {
    // The exact regression v0.0.4/v0.0.5 had: import + comment
    // block survived, the actual `<UpdateBanner />` invocation
    // got dropped. This assertion is the textual guard that would
    // have caught it.
    expect(layoutSrc).toMatch(/<UpdateBanner\s*\/>/);
  });

  it("places UpdateBanner BEFORE <main> so it sits in grid row 2", () => {
    // The component auto-places into the only row without an
    // explicit `row-start-N` (row 2, between the row-start-1
    // header and the row-start-3 main). If a future refactor
    // moves the invocation after `<main>`, CSS auto-placement
    // would route it into row 4 (the footer's slot or beyond) and
    // either hide it under the footer OR push the footer down.
    // Source-order is what makes the auto-placement deterministic.
    const bannerPos = layoutSrc.search(/<UpdateBanner\s*\/>/);
    const mainPos = layoutSrc.search(/<main[\s>]/);
    expect(bannerPos).toBeGreaterThan(0);
    expect(mainPos).toBeGreaterThan(0);
    expect(bannerPos).toBeLessThan(mainPos);
  });
});
