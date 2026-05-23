// Version-string helpers shared by the Settings panel and VersionLink.
//
// Upstream version strings are inconsistent: Sandboxie's installer marker
// stores "5.72.6" (no `v`), Brave's GitHub release tag is "v1.90.124", and
// EVE Vault's release tag is "v0.0.9". Anywhere we want to render a
// `v<digits.dots>` label we have to strip a leading `v` first — otherwise
// the buttons say "Install vv1.92.92" (the bug this helper exists to
// prevent).

/** Strip a single leading `v`/`V` if present. */
export function stripLeadingV(version: string): string {
  return /^v/i.test(version) ? version.slice(1) : version;
}

/** Always-`v<digits>` label for UI buttons and inline version chips.
 *  Tolerates `null` / `undefined` so call-sites don't need to repeat the
 *  `latestVersion ? … : "latest"` ternary — markup branches that have
 *  already proven `latestVersion` truthy still get the formatted label,
 *  and a stray nullish slips through as `"v"` rather than `"vnull"`. */
export function vTag(version: string | null | undefined): string {
  if (!version) return "v";
  return `v${stripLeadingV(version)}`;
}
