//! App-level settings, persisted as JSON in the OS config dir (NOT in the vault,
//! so it never syncs).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// Where the markdown vault lives.
    pub vault: PathBuf,
    /// Use the sred WYSIWYG editor (beta) instead of the raw-markdown TextEdit.
    /// Off by default — the raw editor stays the safe fallback until sred is the
    /// proven default. Editor integration is opt-in.
    #[serde(default)]
    pub wysiwyg_editor: bool,
    /// Remembered window + layout state (0 / empty = unset → use defaults).
    #[serde(default)]
    pub window_w: f32,
    #[serde(default)]
    pub window_h: f32,
    #[serde(default)]
    pub rail_width: f32,
    #[serde(default)]
    pub notes_width: f32,
    #[serde(default = "default_nav_collapsed")]
    pub nav_collapsed: bool,
    #[serde(default)]
    pub last_view: String,
    /// Pinned note ids (bookmarks) shown first in the open-notes tab strip.
    #[serde(default)]
    pub pinned_notes: Vec<String>,
    /// Selected local AI model profile.
    #[serde(default)]
    pub ai_profile: String,
    /// Selected local AI embedding profile.
    #[serde(default)]
    pub ai_embedding_profile: String,
    /// Minimum free memory percentage required before loading a local model.
    #[serde(default)]
    pub ai_min_free_memory_percent: u8,
    /// Maximum seconds a local AI model invocation may run before Noet aborts it.
    #[serde(default)]
    pub ai_timeout_seconds: u64,
    /// Local runtime executable path, usually `mistralrs`.
    #[serde(default)]
    pub ai_runtime_bin: String,
    /// Local model cache/root path for GGUF files.
    #[serde(default)]
    pub ai_model_root: String,
}

fn default_nav_collapsed() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vault: PathBuf::new(),
            wysiwyg_editor: false,
            window_w: 0.0,
            window_h: 0.0,
            rail_width: 0.0,
            notes_width: 0.0,
            nav_collapsed: default_nav_collapsed(),
            last_view: String::new(),
            pinned_notes: Vec::new(),
            ai_profile: String::new(),
            ai_embedding_profile: String::new(),
            ai_min_free_memory_percent: 0,
            ai_timeout_seconds: 0,
            ai_runtime_bin: String::new(),
            ai_model_root: String::new(),
        }
    }
}

impl Settings {
    /// The canonical on-disk location: `<config dir>/noet/settings.json`.
    /// `None` only if the platform exposes no config dir.
    pub fn path() -> Option<PathBuf> {
        if let Ok(dir) = std::env::var("NOET_CONFIG_DIR") {
            return Some(PathBuf::from(dir).join("settings.json"));
        }
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
