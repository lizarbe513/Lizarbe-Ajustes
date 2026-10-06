use std::path::PathBuf;

/// Ubicaciones de Omarchy en disco. `/usr/share/omarchy` solo se lee.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Instalación del paquete Omarchy ($OMARCHY_PATH).
    pub omarchy_path: PathBuf,
    /// Configuración del usuario (~/.config/omarchy).
    pub config_dir: PathBuf,
    /// Tema activo (~/.local/state/omarchy/current/theme).
    pub theme_dir: PathBuf,
    /// Copias de seguridad propias (~/.local/state/lizarbe/backups/widgets).
    pub backup_dir: PathBuf,
    /// Modo sandbox (`--config-dir`): no se habla con el shell en ejecución.
    pub sandbox: bool,
}

impl Paths {
    pub fn detect(config_override: Option<PathBuf>) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let omarchy_path = std::env::var_os("OMARCHY_PATH")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"));
        let state_home = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let sandbox = config_override.is_some();
        let config_dir = config_override.unwrap_or_else(|| home.join(".config/omarchy"));
        let backup_dir = if sandbox {
            config_dir.join(".lizarbe-backups")
        } else {
            state_home.join("lizarbe/backups/widgets")
        };
        Paths {
            omarchy_path,
            config_dir,
            theme_dir: state_home.join("omarchy/current/theme"),
            backup_dir,
            sandbox,
        }
    }

    pub fn shell_json(&self) -> PathBuf {
        self.config_dir.join("shell.json")
    }

    pub fn default_shell_json(&self) -> PathBuf {
        self.omarchy_path.join("config/omarchy/shell.json")
    }

    pub fn shell_toml(&self) -> PathBuf {
        self.config_dir.join("shell.toml")
    }

    pub fn theme_shell_toml(&self) -> PathBuf {
        self.theme_dir.join("shell.toml")
    }

    pub fn theme_colors(&self) -> PathBuf {
        self.theme_dir.join("colors.toml")
    }

    pub fn first_party_plugins(&self) -> PathBuf {
        self.omarchy_path.join("shell/plugins")
    }

    pub fn user_plugins(&self) -> PathBuf {
        self.config_dir.join("plugins")
    }

    pub fn bar_modules(&self) -> PathBuf {
        self.config_dir.join("bar/modules")
    }
}
