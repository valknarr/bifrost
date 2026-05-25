//! Process-wide app state managed by Tauri.
//!
//! Bifrost persists two things to disk: the [`BifrostConfig`] (user
//! settings, last-known Sandboxie path, companion sites) and the list
//! of [`Rider`] records. Both live behind their own `Mutex` so command
//! handlers can hold a lock briefly, snapshot what they need, and drop
//! the lock before any async I/O. Long-running work (Sandboxie CLI
//! shells, GitHub fetches, browser launches) never holds a lock.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Manager};

use crate::config::BifrostConfig;
use crate::error::{BifrostError, Result};
use crate::rider::{Rider, RiderStatus};

/// Mutex-wrapped app state. Owned by Tauri; commands receive it as
/// `State<'_, AppState>`.
pub struct AppState {
    pub config: Mutex<BifrostConfig>,
    pub riders: Mutex<Vec<Rider>>,
    pub config_path: PathBuf,
    pub riders_path: PathBuf,
    pub app_data_dir: PathBuf,
}

impl AppState {
    /// Acquire the riders-list lock, recovering from poisoning.
    ///
    /// A panic inside any other `state.riders.lock()` critical
    /// section would poison the mutex; the default `.unwrap()`
    /// behaviour would then panic EVERY subsequent command,
    /// requiring an app restart. `into_inner` instead takes the
    /// guard's contents (which may be in a partial state, but
    /// it's still a `Vec<Rider>` we can read / mutate); the
    /// alternative — panicking the whole runtime on every call —
    /// is worse for the user.
    ///
    /// All command-site code MUST go through this helper rather
    /// than `state.riders.lock().unwrap()` directly. The latter
    /// pattern is the one that propagated panic across the
    /// codebase before this fix.
    pub fn riders_lock(&self) -> MutexGuard<'_, Vec<Rider>> {
        self.riders.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Acquire the config lock, recovering from poisoning. Same
    /// rationale as [`riders_lock`].
    pub fn config_lock(&self) -> MutexGuard<'_, BifrostConfig> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Build the initial state from disk. Called once during the Tauri
    /// `setup` hook in [`crate::run`].
    pub fn new(app: &AppHandle) -> Result<Self> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|e| BifrostError::Config(format!("no app data dir: {e}")))?;
        std::fs::create_dir_all(&app_data).ok();

        let config_path = app_data.join("config.json");
        let riders_path = app_data.join("riders.json");

        // Wire the release-cache to a backing file under app-data so
        // cached GitHub Releases responses survive app restarts. This
        // is what turns a dev-iteration restart-flurry from "burn 4
        // API calls per restart and hit the 60/hr unauth rate limit
        // after ~15 restarts" into "0 API calls per restart while
        // entries are within their 30-min TTL". See
        // `release_cache::init_disk_cache`.
        crate::release_cache::init_disk_cache(app_data.join("release-cache.json"));

        let config = BifrostConfig::load_or_default(&config_path)?;
        let riders = load_riders(&riders_path)?;

        Ok(Self {
            config: Mutex::new(config),
            riders: Mutex::new(riders),
            config_path,
            riders_path,
            app_data_dir: app_data,
        })
    }

    /// Clone the current config. Most commands only need a snapshot;
    /// they can mutate the clone freely and call [`save_config`] to
    /// persist.
    pub fn config(&self) -> BifrostConfig {
        self.config_lock().clone()
    }

    /// Replace the in-memory config and write it to disk.
    pub fn save_config(&self, new: BifrostConfig) -> Result<()> {
        *self.config_lock() = new.clone();
        new.save(&self.config_path)
    }

    /// Persist the current in-memory rider list to disk. Status fields
    /// are written as-is; on next load they're reset to Stopped since
    /// we can't trust live state across restarts.
    pub fn save_riders(&self) -> Result<()> {
        let riders = self.riders_lock();
        save_riders(&self.riders_path, &riders)
    }

    /// Convenience helper used by [`start_rider`] /
    /// [`stop_rider`](crate::commands::lifecycle) to flip a single
    /// rider's status without touching anything else.
    pub fn set_rider_status(&self, id: &str, status: RiderStatus) {
        let mut riders = self.riders_lock();
        if let Some(p) = riders.iter_mut().find(|p| p.id == id) {
            p.status = status;
        }
    }
}

/// Read `riders.json`. Returns an empty Vec when the file doesn't
/// exist yet. Resets all riders to Stopped on load — runtime state
/// never survives a restart since the sandboxes/processes are gone.
///
/// On parse failure (corrupted JSON, version skew), renames the
/// corrupt file to `<path>.corrupt-<unix>.json` and returns an
/// empty rider list. Mirrors `BifrostConfig::load_or_default`'s
/// fallback shape — better to start with an empty roster the user
/// can rebuild from the still-existing browser-profile dirs and
/// Sandboxie boxes than to refuse to launch with an opaque error.
fn load_riders(path: &Path) -> Result<Vec<Rider>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                "riders: read failed at {}: {e} - falling back to empty roster",
                path.display()
            );
            return Ok(Vec::new());
        }
    };
    let mut riders: Vec<Rider> = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(e) => {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let backup = path.with_extension(format!("corrupt-{ts}.json"));
            tracing::warn!(
                "riders: parse failed at {}: {e} - moved to {} and reset to empty",
                path.display(),
                backup.display()
            );
            let _ = std::fs::rename(path, &backup);
            return Ok(Vec::new());
        }
    };
    for p in riders.iter_mut() {
        p.status = RiderStatus::Stopped;
    }
    Ok(riders)
}

fn save_riders(path: &Path, riders: &[Rider]) -> Result<()> {
    // riders.json is the source of truth for the user's entire
    // roster — a power loss / crash mid-write previously left a
    // zero-byte file and `load_or_default` silently falling back to
    // an empty rider list. Now goes through write_atomic (tmp +
    // rename), which is atomic on every platform we support.
    let json = serde_json::to_string_pretty(riders)?;
    crate::atomic_write::write_atomic(path, json.as_bytes())
}
