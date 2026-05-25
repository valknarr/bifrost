// `vTag()` is the helper that exists specifically to prevent the
// "vv0.0.3" display bug from v0.0.2. The original Brave / EVE Vault
// version-row implementations rendered `v{tag}` where tag itself
// started with `v` — producing `vv1.92.85` (Brave) / `vv0.0.9` (EVE
// Vault) labels in Settings. `vTag` strips one leading `v` and
// re-prepends, so output is always exactly `v<digits>`.
//
// These tests pin the round-trip across all the input shapes the
// helper has to tolerate (raw digits, already-prefixed, uppercase
// prefix, nullish — Tauri command returns are typed as `string |
// null`).

import { describe, expect, it } from "vitest";
import { stripLeadingV, vTag } from "./version";

describe("vTag", () => {
  it("prepends `v` to a raw digit string", () => {
    expect(vTag("0.0.3")).toBe("v0.0.3");
    expect(vTag("1.92.85")).toBe("v1.92.85");
  });

  it("does not double-prepend when the input already starts with v", () => {
    expect(vTag("v0.0.3")).toBe("v0.0.3");
    expect(vTag("v1.92.85")).toBe("v1.92.85");
  });

  it("treats a capital V the same as lowercase v", () => {
    expect(vTag("V1.92")).toBe("v1.92");
  });

  it("returns `v` for nullish input rather than `vnull` or throwing", () => {
    expect(vTag(null)).toBe("v");
    expect(vTag(undefined)).toBe("v");
    expect(vTag("")).toBe("v");
  });
});

describe("stripLeadingV", () => {
  it("strips exactly one leading v (lowercase or uppercase)", () => {
    expect(stripLeadingV("v1.0")).toBe("1.0");
    expect(stripLeadingV("V1.0")).toBe("1.0");
    expect(stripLeadingV("1.0")).toBe("1.0");
  });

  it("does not strip a v anywhere except the first character", () => {
    expect(stripLeadingV("1v0")).toBe("1v0");
    expect(stripLeadingV("vv1.0")).toBe("v1.0"); // only one strip
  });
});
