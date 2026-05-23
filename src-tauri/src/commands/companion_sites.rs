//! Companion-site CRUD. Built-in sites can be disabled but not
//! removed; user-added sites can be removed but don't need the disable
//! affordance.

use tauri::State;

use crate::config::{self, BifrostConfig};
use crate::error::{BifrostError, Result};
use crate::state::AppState;

/// Pure-function validation for new companion-site input. Returns
/// the trimmed name + URL on success; an actionable error on
/// failure. Duplicate-URL detection happens at the command site
/// where the existing list is available — this helper only covers
/// the *shape* of the input.
///
/// Catches:
/// * Empty / whitespace-only name or URL
/// * URLs without an `http://` or `https://` prefix (so a typo
///   like `evefrontier.com` or a `file:///` exfil URL is rejected)
/// * URLs that parse but have no host (`https://`, `https:// `)
///
/// Extracted as `pub(crate)` so command-boundary tests in
/// `mod tests` can exercise it directly without standing up an
/// `AppState`.
pub(crate) fn validate_companion_site_input(name: &str, url: &str) -> Result<(String, String)> {
    let trimmed_name = name.trim().to_string();
    let trimmed_url = url.trim().to_string();
    if trimmed_name.is_empty() || trimmed_url.is_empty() {
        return Err(BifrostError::Other("Name and URL are required.".into()));
    }
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
    Ok((trimmed_name, trimmed_url))
}

/// Append a user-defined companion site to the shared list. Validates
/// input shape via [`validate_companion_site_input`] and
/// duplicate URLs against the existing config.
#[tauri::command]
pub fn add_companion_site(
    state: State<'_, AppState>,
    name: String,
    url: String,
    icon: Option<String>,
) -> Result<BifrostConfig> {
    let (trimmed_name, trimmed_url) = validate_companion_site_input(&name, &url)?;

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
/// — the command silently no-ops for them so the UI doesn't have to
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
/// per-rider Apps row in the UI.
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

#[cfg(test)]
mod tests {
    //! Companion-site validation boundary. `add_companion_site` is
    //! the only command in this module that does input shape
    //! validation; the others (`remove_*`, `set_*_disabled`) are
    //! list-mutation operations covered indirectly.
    use super::*;

    /// Happy path: well-formed http/https URLs + non-empty name
    /// pass validation and return the trimmed pair.
    #[test]
    fn validate_companion_site_input_accepts_well_formed() {
        let (n, u) = validate_companion_site_input("EF Map", "https://ef-map.com/").expect("valid");
        assert_eq!(n, "EF Map");
        assert_eq!(u, "https://ef-map.com/");

        // Whitespace is trimmed, not rejected.
        let (n, u) = validate_companion_site_input("  Docs  ", "  https://docs.example.com  ")
            .expect("valid after trim");
        assert_eq!(n, "Docs");
        assert_eq!(u, "https://docs.example.com");

        // Both http and https are accepted; a localhost URL with
        // port should pass.
        assert!(validate_companion_site_input("Local", "http://localhost:1420/foo").is_ok());
    }

    /// Empty / whitespace name or URL is rejected before scheme
    /// parsing. The error message mentions "required" so the UI
    /// can surface a generic missing-input toast.
    #[test]
    fn validate_companion_site_input_rejects_empty_fields() {
        assert!(validate_companion_site_input("", "https://example.com").is_err());
        assert!(validate_companion_site_input("Name", "").is_err());
        assert!(validate_companion_site_input("   ", "https://example.com").is_err());
        assert!(validate_companion_site_input("Name", "   ").is_err());
        assert!(validate_companion_site_input("", "").is_err());
    }

    /// Non-http(s) schemes are rejected outright — covers the
    /// `file:///etc/passwd` and `javascript:alert(1)` exfil cases.
    #[test]
    fn validate_companion_site_input_rejects_non_http_schemes() {
        assert!(validate_companion_site_input("X", "file:///etc/passwd").is_err());
        assert!(validate_companion_site_input("X", "javascript:alert(1)").is_err());
        assert!(validate_companion_site_input("X", "ftp://example.com").is_err());
        assert!(validate_companion_site_input("X", "data:text/html,<script>").is_err());
        // `chrome-extension://` is used internally for the wallet
        // popup but should NOT be a user-addable companion site.
        assert!(validate_companion_site_input("X", "chrome-extension://abc/popup.html").is_err());
    }

    /// URLs that have the right scheme but parse with no host are
    /// rejected. This is the specific bug the validator was added
    /// to catch — previously `"https:// "` and `"https://"` slipped
    /// through and persisted as garbage.
    #[test]
    fn validate_companion_site_input_rejects_missing_or_empty_host() {
        assert!(validate_companion_site_input("X", "https://").is_err());
        assert!(validate_companion_site_input("X", "http://").is_err());
        // `"https:// "` post-trim is `"https://"` (the trailing
        // space is shaved by validate's `.trim()`), so the
        // "no host" path catches it.
        assert!(validate_companion_site_input("X", "https:// ").is_err());
    }

    /// Malformed URLs (parser errors from `url` crate) are
    /// rejected with a clear error message. We rely on the
    /// parser to do the heavy lifting; assert that the parser
    /// errors propagate at the boundary rather than silently
    /// passing.
    #[test]
    fn validate_companion_site_input_rejects_malformed_urls() {
        // Hosts with control characters / invalid escape.
        assert!(validate_companion_site_input("X", "https://exa mple.com").is_err());
        assert!(validate_companion_site_input("X", "https://[::not-an-ipv6").is_err());
    }
}
