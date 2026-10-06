//! Preferencias propias de meca-qs (~/.config/meca-qs/config.toml).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    /// "es" | "en"; vacío = según el sistema.
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub advanced: bool,
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("meca-qs/config.toml"))
}

impl Prefs {
    pub fn load() -> Prefs {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string(self) {
            let _ = std::fs::write(p, text);
        }
    }
}
