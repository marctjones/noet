//! App-level settings, persisted as JSON in the OS config dir (NOT in the vault,
//! so it never syncs). Currently just the vault location; room to grow defaults.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Where the markdown vault lives.
    pub vault: PathBuf,
    /// Run the Outlook flag/category sync once at app startup (Windows only).
    /// Off by default so the app never launches Outlook/PowerShell unasked.
    #[serde(default)]
    pub outlook_sync_on_open: bool,
}

impl Settings {
    /// The canonical on-disk location: `<config dir>/noet/settings.json`.
    /// `None` only if the platform exposes no config dir.
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("noet").join("settings.json"))
    }

    /// Load settings from the canonical path, or `None` if absent/unreadable.
    pub fn load() -> Option<Settings> {
        Self::load_from(&Self::path()?)
    }

    /// Load settings from an explicit path (used by tests; the canonical [`load`]
    /// delegates here).
    pub fn load_from(path: &Path) -> Option<Settings> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Persist to the canonical path, creating the config dir if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("no OS config dir to store settings.json")?;
        self.save_to(&path)
    }

    /// Persist to an explicit path (used by tests; the canonical [`save`]
    /// delegates here).
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
