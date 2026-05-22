// Backend-error normalisation. Bridge's Tauri commands serialise
// `BridgeError` as a plain string for the frontend (see `error.rs`),
// but JS's `invoke()` then wraps that string into a thrown value
// that stringifies as "Error: <message>". The "Error: " prefix is
// noise — every store / view that catches a backend error wants the
// plain message for display.
//
// Centralising this in one helper means future changes to the error
// wire format (richer payloads, error codes, i18n) only have to land
// here.

/** Strip the noise prefix Tauri adds when serialising a thrown
 *  `BridgeError`, leaving the underlying message ready for display. */
export function formatBackendError(e: unknown): string {
  return String(e).replace(/^Error:\s*/, "");
}
