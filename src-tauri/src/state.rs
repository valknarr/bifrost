//! Process-wide app state managed by Tauri.
//!
//! Bifrost persists two things to disk: the [`BridgeConfig`] (user
//! settings, last-known Sandboxie path, companion sites) and the list
//! of [`Pilot`] records. Both live behind their own `Mutex` so command
//! handlers can hold a lock briefly, snapshot what they need, and drop
//! the lock before any async I/O. Long-running work (Sandboxie CLI
//! shells, GitHub fetches, browser launches) never holds a lock.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::config::BridgeConfig;
use crate::error::{BridgeError, Result};
use crate::pilot::{Pilot, PilotStatus};

/// Mutex-wrapped app state. Owned by Tauri; commands receive it as
/// `State<'_, AppState>`.
pub struct AppState {
    pub config: Mutex<BridgeConfig>,
    pub pilots: Mutex<Vec<Pilot>>,
    pub config_path: PathBuf,
    pub pilots_path: PathBuf,
    pub app_data_dir: PathBuf,
}

impl AppState {
    /// Build the initial state from disk. Called once during the Tauri
    /// `setup` hook in [`crate::run`].
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|e| BridgeError::Config(format!("no app data dir: {e}")))?;
        std::fs::create_dir_all(&app_data).ok();

        let config_path = app_data.join("config.json");
        let pilots_path = app_data.join("pilots.json");

        let config = BridgeConfig::load_or_default(&config_path)?;
        let pilots = load_pilots(&pilots_path)?;

        Ok(Self {
            config: Mutex::new(config),
            pilots: Mutex::new(pilots),
            config_path,
            pilots_path,
            app_data_dir: app_data,
        })
    }

    /// Clone the current config. Most commands only need a snapshot;
    /// they can mutate the clone freely and call [`save_config`] to
    /// persist.
    pub fn config(&self) -> BridgeConfig {
        self.config.lock().unwrap().clone()
    }

    /// Replace the in-memory config and write it to disk.
    pub fn save_config(&self, new: BridgeConfig) -> Result<()> {
        *self.config.lock().unwrap() = new.clone();
        new.save(&self.config_path)
    }

    /// Persist the current in-memory pilot list to disk. Status fields
    /// are written as-is; on next load they're reset to Stopped since
    /// we can't trust live state across restarts.
    pub fn save_pilots(&self) -> Result<()> {
        let pilots = self.pilots.lock().unwrap();
        save_pilots(&self.pilots_path, &pilots)
    }

    /// Convenience helper used by [`start_pilot`] /
    /// [`stop_pilot`](crate::commands::lifecycle) to flip a single
    /// pilot's status without touching anything else.
    pub fn set_pilot_status(&self, id: &str, status: PilotStatus) {
        let mut pilots = self.pilots.lock().unwrap();
        if let Some(p) = pilots.iter_mut().find(|p| p.id == id) {
            p.status = status;
        }
    }
}

/// Read `pilots.json`. Returns an empty Vec when the file doesn't
/// exist yet. Resets all pilots to Stopped on load — runtime state
/// never survives a restart since the sandboxes/processes are gone.
fn load_pilots(path: &Path) -> Result<Vec<Pilot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut pilots: Vec<Pilot> = serde_json::from_str(&raw)?;
    for p in pilots.iter_mut() {
        p.status = PilotStatus::Stopped;
    }
    Ok(pilots)
}

fn save_pilots(path: &Path, pilots: &[Pilot]) -> Result<()> {
    // pilots.json is the source of truth for the user's entire
    // roster — a power loss / crash mid-write previously left a
    // zero-byte file and `load_or_default` silently falling back to
    // an empty pilot list. Now goes through write_atomic (tmp +
    // rename), which is atomic on every platform we support.
    let json = serde_json::to_string_pretty(pilots)?;
    crate::atomic_write::write_atomic(path, json.as_bytes())
}
