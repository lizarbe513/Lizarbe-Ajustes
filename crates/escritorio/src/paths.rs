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

    /// Archivo que generaba Meca HyprConfig.
    pub fn gui_lua(&self) -> PathBuf {
        self.hypr_dir.join("hyprland-gui.lua")
    }

    pub fn bindings_lua(&self) -> PathBuf {
        self.hypr_dir.join("bindings.lua")
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

    /// Carpetas de temas: los del usuario (en pruebas, dentro de la carpeta
    /// de pruebas) y los de Omarchy.
    pub fn theme_dirs(&self) -> crate::themes::Dirs {
        crate::themes::Dirs {
            user: if self.sandbox {
                self.hypr_dir.join("themes")
            } else {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".config/omarchy/themes")
            },
            system: self.omarchy_path.join("themes"),
        }
    }

    /// Nombre del tema que está en uso.
    pub fn current_theme(&self) -> String {
        std::fs::read_to_string(self.theme_dir.with_file_name("theme.name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    pub fn theme_colors(&self) -> PathBuf {
        self.theme_dir.join("colors.toml")
    }
}
