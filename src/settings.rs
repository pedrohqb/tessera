//! Tessera's own settings, a tiny TOML file.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::paths;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub autostart: bool,
    /// Drivers the user switched off.
    #[serde(default)]
    pub disabled_drivers: Vec<String>,
}

impl Settings {
    pub fn load() -> Self {
        fs::read_to_string(paths::settings_file())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = paths::settings_file();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string(self) {
            let _ = fs::write(path, text);
        }
    }
}
