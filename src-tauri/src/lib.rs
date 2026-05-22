//! Bridge backend crate.
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

mod browser;
mod chromium;
mod cmd;
mod commands;
mod config;
mod error;
mod evevault;
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
                .unwrap_or_else(|_| "info,bridge_lib=debug".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Bridge");
}
