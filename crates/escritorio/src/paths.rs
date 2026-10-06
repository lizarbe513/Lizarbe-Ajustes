use std::path::PathBuf;

/// Ubicaciones que usa Escritorio.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Instalación de Omarchy ($OMARCHY_PATH); solo se lee.
    pub omarchy_path: PathBuf,
    /// Configuración de Hyprland del usuario (~/.config/hypr).
    pub hypr_dir: PathBuf,
    /// Tema activo de Omarchy (para los colores de la interfaz).
    pub theme_dir: PathBuf,
    /// Copias de seguridad (~/.local/state/lizarbe/backups/escritorio).
    pub backup_dir: PathBuf,
    /// Modo de pruebas (`--config-dir`): no recarga Hyprland.
    pub sandbox: bool,
}

impl Paths {
    pub fn detect(config_override: Option<PathBuf>) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let state_home = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let sandbox = config_override.is_some();
        let hypr_dir = config_override.unwrap_or_else(|| home.join(".config/hypr"));
        let backup_dir = if sandbox {
            hypr_dir.join(".lizarbe-backups")
        } else {
            state_home.join("lizarbe/backups/escritorio")
        };
        let omarchy_path = std::env::var_os("OMARCHY_PATH")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"));
        Paths {
            omarchy_path,
            hypr_dir,
            theme_dir: state_home.join("omarchy/current/theme"),
            backup_dir,
            sandbox,
        }
    }

    pub fn escritorio_lua(&self) -> PathBuf {
        self.hypr_dir.join("escritorio.lua")
    }

    pub fn hyprland_lua(&self) -> PathBuf {
        self.hypr_dir.join("hyprland.lua")
    }

    pub fn monitors_lua(&self) -> PathBuf {
        self.hypr_dir.join("monitors.lua")
    }

    pub fn bindings_lua(&self) -> PathBuf {
        self.hypr_dir.join("bindings.lua")
    }

    pub fn autostart_lua(&self) -> PathBuf {
        self.hypr_dir.join("autostart.lua")
    }

    pub fn theme_colors(&self) -> PathBuf {
        self.theme_dir.join("colors.toml")
    }
}
