//! Tessera's own settings, a tiny TOML file.

use serde::{Deserialize, Serialize};
use std::fs;

use crate::paths;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub autostart: bool,
    /// No icon in the tray (it shows by default while Tessera runs).
    #[serde(default)]
    pub hide_tray: bool,
    /// Drivers the user switched off.
    #[serde(default)]
    pub disabled_drivers: Vec<String>,
    /// The system's own OpenSC left on: by default Tessera switches it off
    /// for the system's p11-kit (see apps::set_host_opensc_off), from its
    /// first start, and it stays off with Tessera closed.
    #[serde(default)]
    pub system_opensc_on: bool,
}

impl Settings {
    pub fn load() -> Self {
        let mut s: Self = fs::read_to_string(paths::settings_file())
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        // Driver ids are lowercase (an extension's folder may not be).
        for id in &mut s.disabled_drivers {
            *id = id.to_lowercase();
        }
        s
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
