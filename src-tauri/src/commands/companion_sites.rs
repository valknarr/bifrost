//! Companion-site CRUD. Built-in sites can be disabled but not
//! removed; user-added sites can be removed but don't need the disable
//! affordance.

use tauri::State;

use crate::config::{self, BifrostConfig};
use crate::error::{BifrostError, Result};
use crate::state::AppState;

/// Append a user-defined companion site to the shared list. Validates
/// against URL duplication.
#[tauri::command]
pub fn add_companion_site(
    state: State<'_, AppState>,
    name: String,
    url: String,
    icon: Option<String>,
) -> Result<BifrostConfig> {
    let trimmed_name = name.trim().to_string();
    let trimmed_url = url.trim().to_string();
    if trimmed_name.is_empty() || trimmed_url.is_empty() {
        return Err(BifrostError::Other("Name and URL are required.".into()));
    }
    // URL validation. Previously checked only the scheme prefix,
    // which let "https:// " (prefix + space) and "https://" (just
    // the scheme, no host) persist as garbage. Parse with
    // `url::Url::parse` (via reqwest's re-export â€” already a
    // transitive dep) so a missing host, malformed authority, or
    // unsupported scheme all fail at the validation boundary
    // instead of later when something tries to favicon-fetch it
    // or hand it to Tauri's shell-plugin opener.
    if !trimmed_url.starts_with("http://") && !trimmed_url.starts_with("https://") {
        return Err(BifrostError::Other(
            "URL must start with http:// or https://.".into(),
        ));
    }
    let parsed = reqwest::Url::parse(&trimmed_url)
        .map_err(|e| BifrostError::Other(format!("URL is malformed: {e}")))?;
    if parsed.host_str().filter(|h| !h.is_empty()).is_none() {
        return Err(BifrostError::Other(
            "URL must include a host (e.g. https://example.com/).".into(),
        ));
    }

    let mut cfg = state.config();
    if cfg
        .companion_sites
        .iter()
        .any(|s| s.url.eq_ignore_ascii_case(&trimmed_url))
    {
        return Err(BifrostError::Other(format!(
            "A companion site for {trimmed_url} already exists."
        )));
    }

    // Derive a short monogram from the name when the user didn't
    // supply one: first two ASCII-alphanumeric chars, uppercased.
    let derived_icon = icon
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            trimmed_name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(2)
                .collect::<String>()
                .to_uppercase()
        });

    cfg.companion_sites.push(config::CompanionSite {
        name: trimmed_name,
        icon: if derived_icon.is_empty() {
            "?".into()
        } else {
            derived_icon
        },
        url: trimmed_url,
        builtin: false,
        disabled: false,
    });
    state.save_config(cfg.clone())?;
    Ok(cfg)
}

/// Remove a user-added companion site by URL. Built-in sites are kept
/// â€” the command silently no-ops for them so the UI doesn't have to
/// special-case its `Remove` button visibility.
#[tauri::command]
pub fn remove_companion_site(state: State<'_, AppState>, url: String) -> Result<BifrostConfig> {
    let mut cfg = state.config();
    cfg.companion_sites
        // Keep: anything whose URL doesn't match, OR a built-in
        // (built-ins are not removable).
        .retain(|s| !s.url.eq_ignore_ascii_case(&url) || s.builtin);
    state.save_config(cfg.clone())?;
    Ok(cfg)
}

/// Toggle the `disabled` flag on a companion site (typically a
/// built-in, since user-added sites have a `Remove` button instead).
/// Disabled sites stay in the config but are filtered out of the
/// per-pilot Apps row in the UI.
#[tauri::command]
pub fn set_companion_site_disabled(
    state: State<'_, AppState>,
    url: String,
    disabled: bool,
) -> Result<BifrostConfig> {
    let mut cfg = state.config();
    for site in cfg.companion_sites.iter_mut() {
        if site.url.eq_ignore_ascii_case(&url) {
            site.disabled = disabled;
        }
    }
    state.save_config(cfg.clone())?;
    Ok(cfg)
}
