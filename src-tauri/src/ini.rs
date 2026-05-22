//! Minimal Sandboxie.ini parser.
//!
//! Sandboxie's config file is plain text with `[BoxName]` section headers and
//! `Key=Value` lines (one per line). Templates and globals (`[GlobalSettings]`,
//! `[Template_Foo]`) are mixed in with user boxes. We don't need full INI
//! parsing — just an enumeration of user-defined boxes so Bridge can show
//! them in the "Discovered" section.

use std::path::{Path, PathBuf};

use crate::error::Result;

/// One parsed section. Keys are kept in insertion order via Vec because
/// Sandboxie can have repeated keys (`Template=AutoRecoverIgnore`,
/// `Template=BlockPorts`).
#[derive(Debug, Clone, Default)]
struct Section {
    name: String,
    entries: Vec<(String, String)>,
}

impl Section {
    fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn is_enabled(&self) -> bool {
        self.get("Enabled")
            .map(|v| v.eq_ignore_ascii_case("y"))
            .unwrap_or(false)
    }
}

/// Read Sandboxie.ini and return one Section per [Header].
fn parse(path: &Path) -> Result<Vec<Section>> {
    // Sandboxie.ini is UTF-16 LE w/ BOM on most installs; fall back to UTF-8
    // if reading as bytes succeeds but UTF-16 decode fails.
    let bytes = std::fs::read(path)?;
    let text = decode_text(&bytes);

    let mut sections: Vec<Section> = Vec::new();
    let mut current: Option<Section> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if let Some(prev) = current.take() {
                sections.push(prev);
            }
            current = Some(Section {
                name: name.to_string(),
                entries: Vec::new(),
            });
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if let Some(sec) = current.as_mut() {
                sec.entries
                    .push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    if let Some(prev) = current.take() {
        sections.push(prev);
    }
    Ok(sections)
}

fn decode_text(bytes: &[u8]) -> String {
    // UTF-16 LE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    // UTF-16 BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    // UTF-8 BOM (just strip it)
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Candidate locations for Sandboxie.ini, most-recent install first.
fn ini_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(
            PathBuf::from(&local)
                .join("Sandboxie-Plus")
                .join("Sandboxie.ini"),
        );
    }
    if let Ok(roaming) = std::env::var("APPDATA") {
        out.push(
            PathBuf::from(&roaming)
                .join("Sandboxie-Plus")
                .join("Sandboxie.ini"),
        );
    }
    out.push(PathBuf::from(r"C:\Sandbox\Sandboxie.ini"));
    if let Ok(winroot) = std::env::var("SystemRoot") {
        out.push(PathBuf::from(&winroot).join("Sandboxie.ini"));
    }
    out
}

/// Pick the first candidate path that exists.
pub fn locate_ini() -> Option<PathBuf> {
    ini_candidates().into_iter().find(|p| p.exists())
}

/// Convenience: return the names of all user-defined, enabled boxes in
/// the active Sandboxie.ini. Skips `[GlobalSettings]`, `[Template_*]`,
/// `[UserSettings_*]` etc. — only real user boxes.
pub fn list_user_boxes() -> Result<Vec<DiscoveredBox>> {
    let Some(path) = locate_ini() else {
        return Ok(Vec::new());
    };
    let sections = parse(&path)?;
    let mut out = Vec::new();
    for s in sections {
        if is_user_box(&s.name) && s.is_enabled() {
            out.push(DiscoveredBox {
                name: s.name.clone(),
                config_level: s.get("ConfigLevel").map(|v| v.to_string()),
                border_color: s.get("BorderColor").map(|v| v.to_string()),
            });
        }
    }
    Ok(out)
}

fn is_user_box(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !lower.starts_with("globalsettings")
        && !lower.starts_with("template_")
        && !lower.starts_with("usersettings_")
        && !lower.starts_with("certificatesettings")
        && !lower.starts_with("trace")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredBox {
    pub name: String,
    pub config_level: Option<String>,
    pub border_color: Option<String>,
}
