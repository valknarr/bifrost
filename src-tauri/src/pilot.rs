use serde::{Deserialize, Serialize};

/// Lifecycle state of a pilot session. Mirrors the TS type in
/// `src/lib/types.ts` — keep them in sync.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PilotStatus {
    Stopped,
    Starting,
    Running,
    Error,
}

/// One pilot = (Sandboxie box, Chrome profile, optional wallet identity).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pilot {
    /// Stable internal id, e.g. "frontier-1".
    pub id: String,
    /// Display name shown in the UI.
    pub name: String,
    /// Sandboxie box name, e.g. "Frontier1".
    pub sandbox: String,
    /// Per-pilot Chromium `--user-data-dir`. Browser-agnostic; today
    /// Bifrost bundles Brave but the directory format is the standard
    /// Chromium profile layout.
    ///
    /// `serde(alias)` keeps pilots.json files written by pre-v0.1
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
    pub status: PilotStatus,
    /// Hex colour for the accent strip, e.g. "#F39034".
    pub accent: String,
    /// When true, the pilot is hidden from the Managed list and shown in the
    /// Archived section. Sandbox contents are preserved. Restorable.
    #[serde(default)]
    pub archived: bool,
    /// Flips to true the first time Bifrost successfully spawns the game in
    /// this pilot's sandbox. Drives the "first launch will inherit your
    /// default-launcher account" hint ribbon in the UI.
    #[serde(default)]
    pub launched_at_least_once: bool,
}

/// The fixed palette Bifrost cycles pilots through. Each pilot keeps its
/// own accent in `Pilot.accent` so the user can override via the UI.
pub const PALETTE: &[&str] = &[
    "#F39034", // accent orange
    "#3FD17B", // ok green
    "#5AB9FF", // electric blue
    "#C28AFF", // amethyst
    "#F2C94C", // warn yellow
    "#E26060", // danger red
];

impl Pilot {
    /// Build a new pilot stub from a name. The accent is picked as the
    /// first palette colour not already taken by any other pilot
    /// (managed or archived). If the palette is exhausted we just wrap
    /// around to the start — six pilots is more than most multiboxers
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
            // Placeholder — `create_pilot` overrides with a hash name and
            // `adopt_sandbox` overrides with the existing box name.
            sandbox: String::new(),
            browser_profile_dir: String::new(),
            wallet_address: None,
            wallet_balance: None,
            eve_balance: None,
            status: PilotStatus::Stopped,
            accent,
            archived: false,
            launched_at_least_once: false,
        }
    }
}

fn slugify(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    //! Pilot model tests. These deliberately exercise multi-step
    //! sequences (build → serialize → deserialize → compare;
    //! pick-from-palette logic across edge cases) so a regression in
    //! any one piece — slugify, palette ordering, serde derives,
    //! backward-compat aliases — surfaces here rather than at runtime
    //! when a user opens the app and their pilots disappear.
    use super::*;

    /// Happy path: a freshly-built pilot round-trips through JSON
    /// without losing any field. Catches accidental
    /// non-Serialize/Deserialize fields, `#[serde(skip)]` mishaps,
    /// and `rename_all = "camelCase"` drift.
    #[test]
    fn pilot_roundtrips_through_json_lossless() {
        let mut pilot = Pilot::new("Airikr Tuoma", &[]);
        pilot.sandbox = "BifrostABCDEF12".into();
        pilot.browser_profile_dir = "C:/x/y/z".into();
        pilot.wallet_address = Some("0x1234".into());
        pilot.wallet_balance = Some("1.234".into());
        pilot.eve_balance = Some("9999".into());
        pilot.status = PilotStatus::Running;
        pilot.launched_at_least_once = true;
        pilot.archived = true;

        let json = serde_json::to_string(&pilot).expect("serialize");
        let decoded: Pilot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.id, pilot.id);
        assert_eq!(decoded.name, pilot.name);
        assert_eq!(decoded.sandbox, pilot.sandbox);
        assert_eq!(decoded.browser_profile_dir, pilot.browser_profile_dir);
        assert_eq!(decoded.wallet_address, pilot.wallet_address);
        assert_eq!(decoded.wallet_balance, pilot.wallet_balance);
        assert_eq!(decoded.eve_balance, pilot.eve_balance);
        assert_eq!(decoded.status, pilot.status);
        assert_eq!(decoded.accent, pilot.accent);
        assert_eq!(decoded.archived, pilot.archived);
        assert_eq!(decoded.launched_at_least_once, pilot.launched_at_least_once);
    }

    /// Pre-v0.1 `pilots.json` files used `chromeProfileDir` for the
    /// field that's now `browserProfileDir`. The `#[serde(alias)]`
    /// shim on the struct keeps those files readable — if the alias
    /// is ever removed, this test fails before a user's saved
    /// pilots ever vanish.
    #[test]
    fn pilot_with_legacy_chrome_profile_dir_alias_loads() {
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

        let pilot: Pilot = serde_json::from_str(legacy_json).expect("legacy load");
        assert_eq!(pilot.browser_profile_dir, "C:/old/path");
    }

    /// New pilots pick the first palette colour that nobody else is
    /// using. The previous bug was index-based assignment which
    /// collided when archived pilots were counted; this test fixes
    /// the regression.
    #[test]
    fn pilot_new_picks_first_unused_palette_color() {
        let p1 = Pilot::new("First", &[]);
        assert_eq!(p1.accent, PALETTE[0]);

        let p2 = Pilot::new("Second", &[&p1.accent]);
        assert_eq!(p2.accent, PALETTE[1]);

        let p3 = Pilot::new("Third", &[&p1.accent, &p2.accent]);
        assert_eq!(p3.accent, PALETTE[2]);
    }

    /// Palette colours are compared case-insensitively so a user-
    /// typed lowercase override doesn't masquerade as a free slot.
    #[test]
    fn pilot_new_palette_comparison_is_case_insensitive() {
        let taken_lower = PALETTE[0].to_ascii_lowercase();
        let next = Pilot::new("X", &[&taken_lower]);
        assert_eq!(next.accent, PALETTE[1]);
    }

    /// When every palette slot is taken we cycle back to the first
    /// colour. The user can manually override via the accent picker —
    /// this is the deterministic fallback so we never panic / pick
    /// `None`.
    #[test]
    fn pilot_new_cycles_when_palette_exhausted() {
        let taken: Vec<&str> = PALETTE.to_vec();
        let extra = Pilot::new("Extra", &taken);
        assert_eq!(extra.accent, PALETTE[0]);
    }

    /// The slug used as the pilot's stable id must be stripped of
    /// punctuation / whitespace and lowercased. Used in file paths
    /// (`<pilots_dir>/<id>/browser`), so it has to be filesystem-safe.
    #[test]
    fn slugify_makes_ids_filesystem_safe() {
        assert_eq!(slugify("Airikr Tuoma"), "airikr-tuoma");
        assert_eq!(slugify("  Hello, World!  "), "hello--world");
        assert_eq!(slugify("Tal'Ra"), "tal-ra");
    }

    /// The palette must have at least one entry — `Pilot::new` falls
    /// back to `PALETTE[0]` if no unused colour is found, and
    /// indexing into an empty slice would panic.
    #[test]
    fn palette_is_non_empty() {
        assert!(!PALETTE.is_empty());
    }
}
