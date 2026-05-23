//! Tauri command handlers.
//!
//! Each sub-module groups commands by domain so a contributor opening
//! a single file can read all related logic top-to-bottom without
//! scrolling through 1000 lines of unrelated lifecycle code. The
//! [`crate::run`] entry point pulls them together via
//! [`tauri::generate_handler!`].
//!
//! Layout:
//! - [`status`] — `get_status` (host probe summary)
//! - [`config`] — `get_config`, `set_config`
//! - [`riders`] — rider CRUD (create / archive / restore / delete /
//!   accent), plus the helpers `generate_sandbox_name`,
//!   `get_accent_palette`, `list_riders`
//! - [`lifecycle`] — `start_rider`, `stop_rider`, `reconcile_riders`
//! - [`wallet`] — `set_rider_wallet`, plus the shared
//!   `refresh_balances` helper
//! - [`sandboxes`] — discovery + adopt + delete for Sandboxie boxes
//!   that exist outside Bifrost's managed set
//! - [`installers`] — `install_*` / `uninstall_*` / `get_*_status` for
//!   Sandboxie-Plus, Brave, and the EVE Vault extension
//! - [`companion_sites`] — add / remove / disable companion sites
//! - [`browser`] — `open_rider_browser`, `open_rider_app` (wraps the
//!   lower-level [`crate::browser::BrowserLauncher`])

pub mod browser;
pub mod companion_sites;
pub mod config;
pub mod favicon;
pub mod installers;
pub mod lifecycle;
pub mod riders;
pub mod sandboxes;
pub mod status;
pub mod wallet;

// Re-export every command symbol so `crate::commands::*` works as a
// flat list for `tauri::generate_handler![]`.
pub use browser::*;
pub use companion_sites::*;
pub use config::*;
pub use favicon::*;
pub use installers::*;
pub use lifecycle::*;
pub use riders::*;
pub use sandboxes::*;
pub use status::*;
pub use wallet::*;
