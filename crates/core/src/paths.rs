//! Rutas comunes del usuario y de Omarchy.

use std::path::PathBuf;

/// Carpeta personal (o `/` si no se sabe).
pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_STATE_HOME` o `~/.local/state`.
pub fn state_home() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/state"))
}

/// Instalación de Omarchy (`$OMARCHY_PATH` o `/usr/share/omarchy`); solo se lee.
pub fn omarchy() -> PathBuf {
    std::env::var_os("OMARCHY_PATH")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"))
}

/// Tema activo de Omarchy (`~/.local/state/omarchy/current/theme`).
pub fn current_theme() -> PathBuf {
    state_home().join("omarchy/current/theme")
}

/// Copias de seguridad de una aplicación de Lizarbe.
pub fn backups(app: &str) -> PathBuf {
    state_home().join("lizarbe/backups").join(app)
}

/// `~/…` → ruta completa.
pub fn expand(path: &str) -> PathBuf {
    if path == "~" {
        home()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(path)
    }
}
