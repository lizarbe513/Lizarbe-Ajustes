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
        use lizarbe_core::paths as p;
        let sandbox = config_override.is_some();
        let hypr_dir = config_override.unwrap_or_else(|| p::home().join(".config/hypr"));
        let backup_dir = if sandbox {
            hypr_dir.join(".lizarbe-backups")
        } else {
            p::backups("escritorio")
        };
        Paths {
            omarchy_path: p::omarchy(),
            hypr_dir,
            theme_dir: p::current_theme(),
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

    /// Archivo que generaba Meca HyprConfig.
    pub fn gui_lua(&self) -> PathBuf {
        self.hypr_dir.join("hyprland-gui.lua")
    }

    pub fn bindings_lua(&self) -> PathBuf {
        self.hypr_dir.join("bindings.lua")
    }

    /// Variables de entorno de la sesión (uwsm); en pruebas, dentro de la
    /// carpeta de pruebas.
    pub fn uwsm_env(&self) -> PathBuf {
        if self.sandbox {
            self.hypr_dir.join("uwsm-env")
        } else {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".config/uwsm/env")
        }
    }

    pub fn autostart_lua(&self) -> PathBuf {
        self.hypr_dir.join("autostart.lua")
    }

    pub fn hyprsunset_conf(&self) -> PathBuf {
        self.hypr_dir.join("hyprsunset.conf")
    }

    /// `~/.XCompose` (en pruebas, dentro de la carpeta de pruebas).
    pub fn xcompose(&self) -> PathBuf {
        if self.sandbox {
            self.hypr_dir.join(".XCompose")
        } else {
            dirs::home_dir().unwrap_or_default().join(".XCompose")
        }
    }

    pub fn theme_colors(&self) -> PathBuf {
        self.theme_dir.join("colors.toml")
    }
}
