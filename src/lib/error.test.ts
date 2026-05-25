// `formatBackendError` exists to strip the noisy "Error: " prefix
// Tauri adds when an `invoke()` call rejects with a string-shaped
// payload. The helper is small but load-bearing: every store reads
// it on every command failure. Tests pin the round-trip across the
// common input shapes — JS Error, string with prefix, string without,
// nested prefix, non-stringly value — so a future refactor (e.g. an
// extra `String(...)` call, or a tighter regex) can't silently
// regress display.

import { describe, expect, it } from "vitest";
import { formatBackendError } from "./error";

describe("formatBackendError", () => {
  it("strips the `Error: ` prefix from a JS Error", () => {
    const err = new Error("Rider not found: x");
    expect(formatBackendError(err)).toBe("Rider not found: x");
  });

  it("strips the prefix from a plain string", () => {
    expect(formatBackendError("Error: Sandboxie missing")).toBe(
      "Sandboxie missing",
    );
  });

  it("strips exactly one prefix — nested `Error: Error: foo` survives the second", () => {
    // Regression guard: a regex of `/^(Error:\s*)+/` would over-strip.
    // We want the visible second "Error:" to remain so the user can
    // see what the original error actually was.
    expect(formatBackendError("Error: Error: nested")).toBe("Error: nested");
  });

  it("returns a plain stringified form for non-Error, non-string inputs", () => {
    expect(formatBackendError({ toString: () => "Plain message" })).toBe(
      "Plain message",
    );
  });

  it("survives null and undefined without throwing", () => {
    expect(formatBackendError(null)).toBe("null");
    expect(formatBackendError(undefined)).toBe("undefined");
  });
});
