//! Thin Tauri-command wrapper over [`crate::favicon::fetch_data_url`].
//!
//! All caching, sanitisation, stale-while-error fallback, and
//! negative-cache logic lives in the favicon module — this file just
//! exposes one frontend-facing endpoint.

use tauri::State;

use crate::error::Result;
use crate::favicon;
use crate::state::AppState;

/// Resolve a companion-site URL to a `data:image/png;base64,…` URL,
/// or null when no favicon is available. The first call for a given
/// host hits the network (via the shared User-Agent'd HTTP client);
/// subsequent calls within the 7-day TTL serve from the on-disk
/// cache. Failures are negatively-cached the same way so a host
/// without a favicon doesn't get re-fetched every Settings open.
#[tauri::command]
pub async fn get_favicon(state: State<'_, AppState>, url: String) -> Result<Option<String>> {
    Ok(favicon::fetch_data_url(&state.app_data_dir, &url).await)
}
