//! Game configuration (window, graphics, input). Loaded from config.ron at startup.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Persistent game settings. Loaded from `config.ron` in the current directory (or next to the binary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    /// Window width in logical pixels.
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    /// Window height in logical pixels.
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Enable vsync (recommended to avoid tearing).
    #[serde(default = "default_true")]
    pub vsync: bool,
    /// Start in fullscreen.
    #[serde(default)]
    pub fullscreen: bool,
    /// Mouse sensitivity multiplier (1.0 = default).
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
}

fn default_window_width() -> u32 {
    1280
}
fn default_window_height() -> u32 {
    720
}
fn default_true() -> bool {
    true
}
fn default_sensitivity() -> f32 {
    1.0
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            window_width: default_window_width(),
            window_height: default_window_height(),
            vsync: default_true(),
            fullscreen: false,
            sensitivity: default_sensitivity(),
        }
    }
}

impl GameConfig {
    /// Load config from `config.ron`. If the file is missing or invalid, returns default config.
    pub fn load() -> Self {
        let path = config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            match ron::from_str(&data) {
                Ok(c) => return c,
                Err(e) => log::warn!("Invalid config at {:?}: {}, using defaults", path, e),
            }
        }
        Self::default()
    }

    /// Save current config to `config.ron`. Logs on error.
    pub fn save(&self) {
        let path = config_path();
        if let Ok(s) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            if let Err(e) = std::fs::write(&path, s) {
                log::warn!("Could not write config to {:?}: {}", path, e);
            }
        }
    }
}

fn config_path() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")).join("config.ron")
}
