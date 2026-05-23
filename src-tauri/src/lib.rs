//! Bifrost backend crate.
//!
//! `lib.rs` is intentionally thin: it declares the sub-modules, sets
//! up logging, and registers every Tauri command in one place via
//! [`tauri::generate_handler!`]. All command logic lives under
//! [`commands`], grouped by domain (pilots, lifecycle, wallet,
//! sandboxes, installers, companion sites, browser launches, status).
//! App state lives in [`state`].
//!
//! See `src/lib/tauri.ts` on the frontend for the matching invoke
//! wrappers — Tauri command names map 1:1 from snake_case Rust to
//! camelCase TypeScript.

mod atomic_write;
mod browser;
mod chromium;
mod cmd;
mod commands;
mod config;
mod error;
mod evevault;
mod favicon;
mod http;
mod ini;
mod pilot;
mod release_cache;
mod sandboxie;
mod sandboxie_installer;
mod state;
mod sui;

use tauri::Manager;

use crate::state::AppState;

/// Main entry point. Called from `main.rs`.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,bifrost_lib=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // Updater plugin: checks a signed JSON manifest on GitHub
        // Releases for a newer .exe (see `tauri.conf.json` ->
        // `plugins.updater.endpoints`). The frontend polls via
        // `@tauri-apps/plugin-updater` and surfaces an "Update
        // available" banner — the user opts in to the relaunch.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Process plugin: only `relaunch()` is granted in the
        // capability set. The updater calls it after the new
        // installer has run so the user lands back on the new
        // version automatically.
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Status / config
            commands::get_status,
            commands::get_config,
            commands::set_config,
            commands::set_ui_zoom,
            commands::set_roster_columns,
            commands::set_roster_window_size,
            // Pilots (CRUD)
            commands::list_pilots,
            commands::create_pilot,
            commands::archive_pilot,
            commands::restore_pilot,
            commands::delete_pilot,
            commands::set_pilot_accent,
            commands::get_accent_palette,
            // Pilots (lifecycle)
            commands::start_pilot,
            commands::stop_pilot,
            commands::reconcile_pilots,
            // Pilots (wallet)
            commands::set_pilot_wallet,
            // Sandboxes (discover / adopt / delete unmanaged)
            commands::detect_sandboxie,
            commands::list_sandboxes,
            commands::adopt_sandbox,
            commands::delete_sandbox,
            // Installers (Sandboxie / Brave / EVE Vault)
            commands::get_evevault_status,
            commands::install_evevault,
            commands::uninstall_evevault,
            commands::get_chromium_status,
            commands::install_chromium,
            commands::uninstall_chromium,
            commands::get_sandboxie_installer_status,
            commands::install_sandboxie,
            commands::uninstall_sandboxie,
            // Per-pilot browser launches
            commands::open_pilot_browser,
            commands::open_pilot_app,
            // Companion sites
            commands::add_companion_site,
            commands::remove_companion_site,
            commands::set_companion_site_disabled,
            commands::get_favicon,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bifrost");
}

#[cfg(test)]
mod tests {
    //! Cross-boundary drift tests for the Rust ↔ TS command surface.
    //!
    //! The Tauri IPC layer is invisible to the type system: a
    //! `#[tauri::command]` added to Rust but forgotten in
    //! `src/lib/tauri.ts` (or vice-versa) compiles cleanly on both
    //! sides and only blows up at runtime when the UI first invokes
    //! the missing wrapper. This test fixture parses both files at
    //! `cargo test` time and asserts the sets are identical — so
    //! the drift is impossible to merge.
    use std::collections::HashSet;

    /// Path to `src/lib/tauri.ts` relative to this crate's
    /// `Cargo.toml`. Resolved at compile time via
    /// `CARGO_MANIFEST_DIR` so CI and local runs both find it.
    const TAURI_TS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/tauri.ts");

    /// Path to this crate's `lib.rs` — we parse the
    /// `generate_handler!` macro arg list out of it.
    const LIB_RS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs");

    /// Extract the set of command names from the
    /// `generate_handler![commands::foo, commands::bar, …]` block in
    /// `lib.rs`. Tolerates comments and whitespace between entries.
    fn rust_commands() -> HashSet<String> {
        let src = std::fs::read_to_string(LIB_RS_PATH).expect("read lib.rs");
        let start = src
            .find("generate_handler![")
            .expect("generate_handler! macro present");
        let after = &src[start..];
        let end = after.find(']').expect("generate_handler! has closing ]");
        let body = &after[..end];

        let mut out = HashSet::new();
        for line in body.lines() {
            let line = line.trim().trim_end_matches(',').trim();
            // Skip comment-only and empty lines.
            if line.is_empty() || line.starts_with("//") || line.starts_with("generate_handler") {
                continue;
            }
            // Format: `commands::snake_case_name`
            if let Some(name) = line.strip_prefix("commands::") {
                let name = name.trim_end_matches(',').trim();
                if !name.is_empty() {
                    out.insert(name.to_string());
                }
            }
        }
        out
    }

    /// Extract the set of command names from `tauri.ts`. We grep
    /// every `invoke<...>("snake_case")` occurrence — the string
    /// literal is the Tauri-side name, which is what Rust receives.
    fn ts_commands() -> HashSet<String> {
        let src = std::fs::read_to_string(TAURI_TS_PATH).expect("read tauri.ts");
        let mut out = HashSet::new();
        // Look for `invoke<...>("name"`. The closing `"` is the
        // delimiter — find each open quote after `invoke<` and read
        // until the next quote.
        let mut rest = src.as_str();
        while let Some(idx) = rest.find("invoke<") {
            // After `invoke<`, skip the type param + `>(` + maybe
            // whitespace, find the first opening quote.
            let after = &rest[idx..];
            let Some(open) = after.find('"') else { break };
            let name_start = idx + open + 1;
            let Some(close) = rest[name_start..].find('"') else {
                break;
            };
            let name = &rest[name_start..name_start + close];
            // Tauri command names are snake_case identifiers — defensive
            // filter so we don't accidentally pick up other quoted
            // strings further down the line.
            if name
                .chars()
                .all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
                && !name.is_empty()
            {
                out.insert(name.to_string());
            }
            // Advance past this invoke< occurrence.
            rest = &rest[name_start + close + 1..];
        }
        out
    }

    /// The two sets MUST be equal. If they differ, the failure
    /// message lists each side's exclusive entries so a contributor
    /// can see exactly which command to add / remove.
    #[test]
    fn rust_and_ts_command_surfaces_match() {
        let rust = rust_commands();
        let ts = ts_commands();

        assert!(!rust.is_empty(), "no Rust commands parsed — parser drift?");
        assert!(!ts.is_empty(), "no TS commands parsed — parser drift?");

        let rust_only: Vec<&str> = rust.difference(&ts).map(|s| s.as_str()).collect();
        let ts_only: Vec<&str> = ts.difference(&rust).map(|s| s.as_str()).collect();

        assert!(
            rust_only.is_empty() && ts_only.is_empty(),
            "\nCommand surface drift between Rust and TypeScript:\n\
             - Defined in Rust but missing TS wrapper in src/lib/tauri.ts: {:?}\n\
             - Defined in TS but no #[tauri::command] in lib.rs: {:?}\n\
             Fix the missing side, OR remove the orphan, so the surface stays in sync.",
            rust_only,
            ts_only
        );
    }

    /// Pin the parser's "lower bound" against accidental regressions
    /// in the helpers above: if the parser breaks and silently
    /// returns an empty set, `rust_and_ts_command_surfaces_match`
    /// would pass trivially (empty == empty). This test makes the
    /// parser's coverage explicit.
    #[test]
    fn command_surface_parsers_find_a_reasonable_count() {
        let rust = rust_commands();
        let ts = ts_commands();
        // We have ~36 commands today; assert > 20 in both to catch
        // a parser regression that quietly halves the count.
        assert!(
            rust.len() > 20,
            "Rust parser returned only {} commands — parser regression?",
            rust.len()
        );
        assert!(
            ts.len() > 20,
            "TS parser returned only {} commands — parser regression?",
            ts.len()
        );
    }
}
