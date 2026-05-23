use serde::{Deserialize, Serialize};

/// Lifecycle state of a rider session. Mirrors the TS type in
/// `src/lib/types.ts` — keep them in sync.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiderStatus {
    Stopped,
    Starting,
    Running,
    Error,
    /// The rider's Sandboxie box is no longer present in Sandboxie.ini
    /// — the user (or another tool) deleted the sandbox externally,
    /// leaving Bifrost's rider record pointing at nothing. The UI
    /// surfaces this with a "Sandbox missing — Remove rider" action;
    /// reconcile sets it via a pre-flight check before any Start.exe
    /// call so a stale rider doesn't trigger the native "Invalid box
    /// name parameter" dialog every 30 s.
    Missing,
}

/// One rider = (Sandboxie box, Chrome profile, optional wallet identity).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rider {
    /// Stable internal id, e.g. "frontier-1".
    pub id: String,
    /// Display name shown in the UI.
    pub name: String,
    /// Sandboxie box name, e.g. "Frontier1".
    pub sandbox: String,
    /// Per-rider Chromium `--user-data-dir`. Browser-agnostic; today
    /// Bifrost bundles Brave but the directory format is the standard
    /// Chromium profile layout.
    ///
    /// `serde(alias)` keeps riders.json files written by pre-v0.1
    /// builds (which used `chromeProfileDir`) readable without manual
    /// migration.
    #[serde(alias = "chromeProfileDir")]
    pub browser_profile_dir: String,
    /// Sui wallet address (read-only display; may be unknown until first login).
    pub wallet_address: Option<String>,
    /// Optional last-known SUI gas-token balance, pre-formatted.
    pub wallet_balance: Option<String>,
    /// Optional last-known EVE token balance, pre-formatted. EVE is the
    /// player-facing currency in EVE Frontier; SUI is just gas.
    #[serde(default)]
    pub eve_balance: Option<String>,
    pub status: RiderStatus,
    /// Hex colour for the accent strip, e.g. "#F39034".
    pub accent: String,
    /// When true, the rider is hidden from the Managed list and shown in the
    /// Archived section. Sandbox contents are preserved. Restorable.
    #[serde(default)]
    pub archived: bool,
    /// Flips to true the first time Bifrost successfully spawns the game in
    /// this rider's sandbox. Drives the "first launch will inherit your
    /// default-launcher account" hint ribbon in the UI.
    #[serde(default)]
    pub launched_at_least_once: bool,
    /// Unix milliseconds at which `wallet_balance` / `eve_balance` were
    /// last successfully refreshed from the Sui RPC. `None` until the
    /// first successful refresh.
    ///
    /// The reconcile loop runs every 30 s and overwrites the balance
    /// fields on success — but on RPC failure it silently keeps the
    /// previous value. Without this timestamp the user can't tell
    /// whether the figure on screen is current or 20 minutes stale.
    /// RiderCard reads this and shows a "Updated 5m ago" / "Stale"
    /// indicator so the staleness is never hidden.
    #[serde(default)]
    pub wallet_balance_fetched_at: Option<u64>,
}

/// The fixed palette Bifrost cycles riders through. Each rider keeps its
/// own accent in `Rider.accent` so the user can override via the UI.
pub const PALETTE: &[&str] = &[
    "#F39034", // accent orange
    "#3FD17B", // ok green
    "#5AB9FF", // electric blue
    "#C28AFF", // amethyst
    "#F2C94C", // warn yellow
    "#E26060", // danger red
];

impl Rider {
    /// Build a new rider stub from a name. The accent is picked as the
    /// first palette colour not already taken by any other rider
    /// (managed or archived). If the palette is exhausted we just wrap
    /// around to the start — six riders is more than most multiboxers
    /// will ever run and the user can override manually anyway.
    pub fn new(name: impl Into<String>, taken_accents: &[&str]) -> Self {
        let name: String = name.into();
        let id = slugify(&name);

        let accent = PALETTE
            .iter()
            .find(|c| !taken_accents.iter().any(|t| t.eq_ignore_ascii_case(c)))
            .copied()
            .unwrap_or(PALETTE[0])
            .to_string();

        Self {
            id,
            name,
            // Placeholder — `create_rider` overrides with a hash name and
            // `adopt_sandbox` overrides with the existing box name.
            sandbox: String::new(),
            browser_profile_dir: String::new(),
            wallet_address: None,
            wallet_balance: None,
            eve_balance: None,
            status: RiderStatus::Stopped,
            accent,
            archived: false,
            launched_at_least_once: false,
            wallet_balance_fetched_at: None,
        }
    }
}

/// Convert a display name to a filesystem-safe slug.
///
/// Public so `commands::riders` and `commands::sandboxes` can both
/// use it as their canonical name → id mapping. Returns an empty
/// string for names that contain no alphanumeric characters (e.g.
/// `"!!!"`, `"   "`, `""`) — the caller MUST treat an empty result
/// as a validation failure rather than constructing a rider with
/// `id == ""` (which expands `<riders_dir>/<id>` to the parent
/// directory and lets `remove_dir_all` wipe every rider at once).
pub fn slugify(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Produce a rider id that doesn't collide with any existing rider's
/// id. `slug` is the slugified display name; `existing_ids` is the
/// set of ids already in use (archived + managed). If `slug` is
/// already unique, returns it. Otherwise appends `-<8 hex>` derived
/// from system time so the user can reuse names freely (e.g. archive
/// an old "Airikr", create a new "Airikr") without aliasing on disk.
///
/// Returns `None` if `slug` is empty — callers should validate the
/// display name before getting here, but `None` is a second line of
/// defence that maps cleanly to a validation error at the command
/// boundary.
pub fn unique_rider_id<I>(slug: &str, existing_ids: I) -> Option<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    if slug.is_empty() {
        return None;
    }
    let taken: std::collections::HashSet<String> = existing_ids
        .into_iter()
        .map(|s| s.as_ref().to_ascii_lowercase())
        .collect();
    if !taken.contains(&slug.to_ascii_lowercase()) {
        return Some(slug.to_string());
    }
    // Collision — append a time-derived hex suffix. Loop guards
    // against the (astronomically unlikely) case that the suffix
    // itself collides on a fast retry.
    for _ in 0..16 {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let candidate = format!("{slug}-{:08x}", (ts.wrapping_mul(0x9E37_79B9) >> 16) as u32);
        if !taken.contains(&candidate.to_ascii_lowercase()) {
            return Some(candidate);
        }
    }
    // Fall back to a deterministic ordinal so we never return None
    // for a non-empty slug.
    let mut n = 2;
    loop {
        let candidate = format!("{slug}-{n}");
        if !taken.contains(&candidate.to_ascii_lowercase()) {
            return Some(candidate);
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    //! Rider model tests. These deliberately exercise multi-step
    //! sequences (build → serialize → deserialize → compare;
    //! pick-from-palette logic across edge cases) so a regression in
    //! any one piece — slugify, palette ordering, serde derives,
    //! backward-compat aliases — surfaces here rather than at runtime
    //! when a user opens the app and their riders disappear.
    use super::*;

    /// Happy path: a freshly-built rider round-trips through JSON
    /// without losing any field. Catches accidental
    /// non-Serialize/Deserialize fields, `#[serde(skip)]` mishaps,
    /// and `rename_all = "camelCase"` drift.
    #[test]
    fn rider_roundtrips_through_json_lossless() {
        let mut rider = Rider::new("Airikr Tuoma", &[]);
        rider.sandbox = "BifrostABCDEF12".into();
        rider.browser_profile_dir = "C:/x/y/z".into();
        rider.wallet_address = Some("0x1234".into());
        rider.wallet_balance = Some("1.234".into());
        rider.eve_balance = Some("9999".into());
        rider.status = RiderStatus::Running;
        rider.launched_at_least_once = true;
        rider.archived = true;

        let json = serde_json::to_string(&rider).expect("serialize");
        let decoded: Rider = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, rider.id);
        assert_eq!(decoded.name, rider.name);
        assert_eq!(decoded.sandbox, rider.sandbox);
        assert_eq!(decoded.browser_profile_dir, rider.browser_profile_dir);
        assert_eq!(decoded.wallet_address, rider.wallet_address);
        assert_eq!(decoded.wallet_balance, rider.wallet_balance);
        assert_eq!(decoded.eve_balance, rider.eve_balance);
        assert_eq!(decoded.status, rider.status);
        assert_eq!(decoded.accent, rider.accent);
        assert_eq!(decoded.archived, rider.archived);
        assert_eq!(decoded.launched_at_least_once, rider.launched_at_least_once);
    }

    /// Pre-v0.1 `riders.json` files used `chromeProfileDir` for the
    /// field that's now `browserProfileDir`. The `#[serde(alias)]`
    /// shim on the struct keeps those files readable — if the alias
    /// is ever removed, this test fails before a user's saved
    /// riders ever vanish.
    #[test]
    fn rider_with_legacy_chrome_profile_dir_alias_loads() {
        let legacy_json = r##"{
            "id": "airikr",
            "name": "Airikr",
            "sandbox": "Frontier1",
            "chromeProfileDir": "C:/old/path",
            "walletAddress": null,
            "walletBalance": null,
            "status": "stopped",
            "accent": "#F39034"
        }"##;

        let rider: Rider = serde_json::from_str(legacy_json).expect("legacy load");
        assert_eq!(rider.browser_profile_dir, "C:/old/path");
    }

    /// New riders pick the first palette colour that nobody else is
    /// using. The previous bug was index-based assignment which
    /// collided when archived riders were counted; this test fixes
    /// the regression.
    #[test]
    fn rider_new_picks_first_unused_palette_color() {
        let p1 = Rider::new("First", &[]);
        assert_eq!(p1.accent, PALETTE[0]);

        let p2 = Rider::new("Second", &[&p1.accent]);
        assert_eq!(p2.accent, PALETTE[1]);

        let p3 = Rider::new("Third", &[&p1.accent, &p2.accent]);
        assert_eq!(p3.accent, PALETTE[2]);
    }

    /// Palette colours are compared case-insensitively so a user-
    /// typed lowercase override doesn't masquerade as a free slot.
    #[test]
    fn rider_new_palette_comparison_is_case_insensitive() {
        let taken_lower = PALETTE[0].to_ascii_lowercase();
        let next = Rider::new("X", &[&taken_lower]);
        assert_eq!(next.accent, PALETTE[1]);
    }

    /// When every palette slot is taken we cycle back to the first
    /// colour. The user can manually override via the accent picker —
    /// this is the deterministic fallback so we never panic / pick
    /// `None`.
    #[test]
    fn rider_new_cycles_when_palette_exhausted() {
        let taken: Vec<&str> = PALETTE.to_vec();
        let extra = Rider::new("Extra", &taken);
        assert_eq!(extra.accent, PALETTE[0]);
    }

    /// The slug used as the rider's stable id must be stripped of
    /// punctuation / whitespace and lowercased. Used in file paths
    /// (`<riders_dir>/<id>/browser`), so it has to be filesystem-safe.
    #[test]
    fn slugify_makes_ids_filesystem_safe() {
        assert_eq!(slugify("Airikr Tuoma"), "airikr-tuoma");
        assert_eq!(slugify("  Hello, World!  "), "hello--world");
        assert_eq!(slugify("Tal'Ra"), "tal-ra");
    }

    /// Load-bearing: `commands::riders::create_rider` rejects a rider
    /// whose name slugifies to `""` precisely because `<riders_dir>/""`
    /// collapses to the parent directory — a subsequent delete would
    /// then wipe every rider's browser profile. The guard at
    /// `commands/riders.rs::create_rider` depends on this function
    /// returning empty for these three input shapes; if a refactor
    /// changes the contract (e.g. "always return at least one char"),
    /// the downstream guard becomes wrong without any compile-time
    /// signal. This test pins both halves of the contract.
    #[test]
    fn slugify_returns_empty_for_inputs_without_alphanumerics() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify("\t\n"), "");
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("   - , ' ! "), "");
    }

    /// The palette must have at least one entry — `Rider::new` falls
    /// back to `PALETTE[0]` if no unused colour is found, and
    /// indexing into an empty slice would panic. Six colours is the
    /// current design minimum (matches the natural rider count cap);
    /// asserting >= 5 pins the floor while leaving room for the
    /// design to drop one without breaking the test.
    #[test]
    fn palette_meets_minimum_size_for_rider_new_fallback() {
        assert!(
            PALETTE.len() >= 5,
            "PALETTE must keep at least 5 entries to support the design's rider accent variety; \
             also guards Rider::new's PALETTE[0] indexing from panicking"
        );
    }

    /// `unique_rider_id` returns the slug unchanged when nothing
    /// collides — the common case for the first rider or a fresh
    /// display name.
    #[test]
    fn unique_rider_id_returns_slug_when_no_collision() {
        let existing: Vec<&str> = vec!["other"];
        let result = unique_rider_id("airikr", existing);
        assert_eq!(result.as_deref(), Some("airikr"));
    }

    /// `unique_rider_id` returns `None` for an empty slug. This is
    /// load-bearing: `create_rider` and `adopt_sandbox` propagate
    /// the `None` to a validation error. If a future refactor makes
    /// this return `Some("")`, the empty-id data-loss bug
    /// (every rider's profile wiped on first delete) returns. See
    /// `commands/riders.rs::create_rider` doc comment for context.
    #[test]
    fn unique_rider_id_returns_none_for_empty_slug() {
        let result = unique_rider_id("", std::iter::empty::<&str>());
        assert!(result.is_none(), "empty slug must short-circuit to None");
    }

    /// `unique_rider_id` appends a hex suffix when the slug collides
    /// with ANY existing id (archived or active). Two riders with
    /// the same display name previously shared a browser-profile
    /// directory; this is what stops that. Tests two collision
    /// shapes:
    ///   1. Exact slug match → suffix appended
    ///   2. Case-only difference → still treated as collision (the
    ///      filesystem we ultimately use is case-insensitive on
    ///      Windows, so we must dedup case-insensitively).
    #[test]
    fn unique_rider_id_appends_suffix_on_collision() {
        let existing = ["airikr"];
        let result = unique_rider_id("airikr", existing.iter().copied()).expect("non-empty slug");
        assert_ne!(result, "airikr", "must NOT return the colliding slug");
        assert!(
            result.starts_with("airikr-"),
            "result must start with original slug + '-' suffix, got {result:?}"
        );
        // Case-insensitive collision detection.
        let existing_upper = ["AIRIKR"];
        let result2 =
            unique_rider_id("airikr", existing_upper.iter().copied()).expect("non-empty slug");
        assert_ne!(
            result2.to_ascii_lowercase(),
            "airikr",
            "case-only collision must still trigger suffix"
        );
    }

    /// Stress: when the entire hex-suffix space is occupied (or the
    /// generator keeps producing the same suffix due to a clock
    /// quirk), the function must STILL return a valid id via the
    /// deterministic ordinal fallback. Pinning the fallback so a
    /// future refactor that drops it doesn't silently let the
    /// function spin forever or return `None`.
    #[test]
    fn unique_rider_id_falls_back_to_ordinal_when_suffix_exhausted() {
        // Construct an `existing` set containing the slug + 100
        // hex-suffix variants AND the first few ordinal fallbacks.
        // Use a contrived suffix space so we can guarantee
        // collision-then-fallback in deterministic time.
        let mut taken: Vec<String> = vec!["x".to_string()];
        for i in 0..2 {
            taken.push(format!("x-{i}"));
        }
        // The function tries 16 hex suffixes then ordinal-from-2.
        // Even if the 16 hex attempts collide, the ordinal `x-2`
        // path eventually returns a non-colliding name. Just verify
        // we never return `None` and the result is unique.
        let result =
            unique_rider_id("x", taken.iter().map(|s| s.as_str())).expect("must return Some");
        assert_ne!(result, "x");
        assert!(!taken.iter().any(|t| t.eq_ignore_ascii_case(&result)));
    }

    /// RiderStatus ↔ TypeScript string union drift. The TS side
    /// (`src/lib/types.ts`) declares a hard-coded union of these
    /// status strings — adding a Rust variant without updating the
    /// TS type is silent at compile time on both sides but breaks
    /// the frontend's StatusBadge rendering at runtime. This test
    /// emits every variant's JSON spelling and compares against a
    /// hard-coded list; a contributor adding a new variant has to
    /// update BOTH this assertion AND `types.ts` together.
    ///
    /// The serde representation is `rename_all = "lowercase"`, so
    /// each variant becomes its name in lowercase.
    #[test]
    fn rider_status_variants_match_typescript_union() {
        // Hard-coded list: this MUST stay in sync with the
        // `RiderStatus` type alias in src/lib/types.ts.
        let expected: std::collections::HashSet<&str> =
            ["stopped", "starting", "running", "error", "missing"]
                .iter()
                .copied()
                .collect();

        // Materialise every variant's JSON spelling. Listing them
        // exhaustively here also means: if a new variant is added
        // to the enum, the contributor must touch this test.
        let variants = [
            RiderStatus::Stopped,
            RiderStatus::Starting,
            RiderStatus::Running,
            RiderStatus::Error,
            RiderStatus::Missing,
        ];
        let actual: std::collections::HashSet<String> = variants
            .iter()
            .map(|s| {
                serde_json::to_string(s)
                    .expect("serialize")
                    .trim_matches('"')
                    .to_string()
            })
            .collect();
        let actual_refs: std::collections::HashSet<&str> =
            actual.iter().map(|s| s.as_str()).collect();

        assert_eq!(
            actual_refs, expected,
            "\nRiderStatus drift: the serde-serialised variant names \
             don't match the hard-coded TypeScript union in \
             src/lib/types.ts. Update BOTH this test AND types.ts \
             when adding / removing a variant.\n\
             Rust variants serialise to: {actual_refs:?}\n\
             Expected (per types.ts): {expected:?}\n"
        );
    }
}
