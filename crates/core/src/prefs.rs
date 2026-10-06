//! Preferencias compartidas por las TUIs Lizarbe
//! (`~/.config/lizarbe/ajustes.toml`): idioma y modo simple/avanzado.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Prefs {
    /// "es" | "en"; vacío = según el sistema.
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub advanced: bool,
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("lizarbe/ajustes.toml"))
}

/// Ubicación anterior, de cuando Widgets se llamaba meca-qs.
fn legacy_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("meca-qs/config.toml"))
}

fn read(p: &Path) -> Option<Prefs> {
    toml::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

impl Prefs {
    pub fn load() -> Prefs {
        match (path(), legacy_path()) {
            (Some(p), Some(legacy)) => Prefs::load_from(&p, &legacy),
            _ => Prefs::default(),
        }
    }

    /// Lee `p`; si no existe, adopta (una sola vez) las preferencias de
    /// `legacy` y las guarda en `p`.
    pub fn load_from(p: &Path, legacy: &Path) -> Prefs {
        if p.exists() {
            return read(p).unwrap_or_default();
        }
        match read(legacy) {
            Some(prefs) => {
                prefs.save_to(p);
                prefs
            }
            None => Prefs::default(),
        }
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            self.save_to(&p);
        }
    }

    pub fn save_to(&self, p: &Path) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string(self) {
            let _ = crate::fsutil::write_atomic(p, &text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_once() {
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("lizarbe/ajustes.toml");
        let legacy = dir.path().join("meca-qs/config.toml");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, "lang = \"es\"\nadvanced = true\n").unwrap();

        let p = Prefs::load_from(&new, &legacy);
        assert_eq!(p.lang, "es");
        assert!(p.advanced);
        assert!(new.exists(), "se guarda en la ubicación nueva");

        // Desde entonces manda el archivo nuevo.
        std::fs::write(&legacy, "lang = \"en\"\n").unwrap();
        assert_eq!(Prefs::load_from(&new, &legacy).lang, "es");
    }

    #[test]
    fn defaults_without_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = Prefs::load_from(&dir.path().join("a.toml"), &dir.path().join("b.toml"));
        assert_eq!(p, Prefs::default());
    }
}
