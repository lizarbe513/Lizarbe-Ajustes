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
        use lizarbe_core::paths as p;
        let home = p::home();
        let omarchy_path = p::omarchy();
        let sandbox = config_override.is_some();
        let config_dir = config_override.unwrap_or_else(|| home.join(".config/omarchy"));
        let backup_dir = if sandbox {
            config_dir.join(".lizarbe-backups")
        } else {
            p::backups("widgets")
        };
        Paths {
            omarchy_path,
            config_dir,
            theme_dir: p::current_theme(),
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
