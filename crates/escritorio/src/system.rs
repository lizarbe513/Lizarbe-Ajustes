//! Sistema: apps por defecto, perfil de energía, fuente y accesos a los
//! paneles del shell de Omarchy. No guarda nada propio: lee y cambia el
//! estado con los scripts `omarchy-*`, y los cambios valen al momento (no
//! pasan por Aplicar).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lizarbe_core::ipc;
use lizarbe_core::term::Command;

use crate::i18n::t;

/// Lo que se elige de una lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pick {
    Browser,
    Terminal,
    Editor,
    Power,
    Font,
    TextSize,
}

/// Cómo saber si una opción está instalada.
enum Check {
    /// Archivo `.desktop`.
    Desktop(&'static str),
    /// Alguno de estos programas.
    Bin(&'static [&'static str]),
}

/// Opciones que aceptan los scripts `omarchy-default-*`: (valor, nombre, cómo
/// saber si está instalada).
const BROWSERS: &[(&str, &str, Check)] = &[
    ("chromium", "Chromium", Check::Desktop("chromium.desktop")),
    (
        "chrome",
        "Google Chrome",
        Check::Desktop("google-chrome.desktop"),
    ),
    ("brave", "Brave", Check::Desktop("brave-browser.desktop")),
    (
        "brave-origin",
        "Brave Origin",
        Check::Desktop("brave-origin.desktop"),
    ),
    (
        "edge",
        "Microsoft Edge",
        Check::Desktop("microsoft-edge.desktop"),
    ),
    ("firefox", "Firefox", Check::Desktop("firefox.desktop")),
    ("zen", "Zen", Check::Desktop("zen.desktop")),
];

const TERMINALS: &[(&str, &str, Check)] = &[
    ("alacritty", "Alacritty", Check::Bin(&["alacritty"])),
    ("foot", "Foot", Check::Bin(&["foot"])),
    ("ghostty", "Ghostty", Check::Bin(&["ghostty"])),
    ("kitty", "Kitty", Check::Bin(&["kitty"])),
];

const EDITORS: &[(&str, &str, Check)] = &[
    ("nvim", "Neovim", Check::Bin(&["nvim"])),
    ("code", "VSCode", Check::Bin(&["code"])),
    ("cursor", "Cursor", Check::Bin(&["cursor"])),
    ("zed", "Zed", Check::Bin(&["zeditor", "zed"])),
    (
        "sublime_text",
        "Sublime Text",
        Check::Bin(&["sublime_text", "subl"]),
    ),
    ("helix", "Helix", Check::Bin(&["helix", "hx"])),
    ("vim", "Vim", Check::Bin(&["vim"])),
    ("emacs", "Emacs", Check::Bin(&["emacs"])),
];

impl Pick {
    pub const ALL: [Pick; 6] = [
        Pick::Browser,
        Pick::Terminal,
        Pick::Editor,
        Pick::Power,
        Pick::Font,
        Pick::TextSize,
    ];

    /// Clave de texto (`sys.pick.<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Pick::Browser => "browser",
            Pick::Terminal => "terminal",
            Pick::Editor => "editor",
            Pick::Power => "power",
            Pick::Font => "font",
            Pick::TextSize => "text_size",
        }
    }

    fn table(self) -> &'static [(&'static str, &'static str, Check)] {
        match self {
            Pick::Browser => BROWSERS,
            Pick::Terminal => TERMINALS,
            Pick::Editor => EDITORS,
            Pick::Power | Pick::Font | Pick::TextSize => &[],
        }
    }

    /// Valor en uso según Omarchy.
    fn current(self) -> Option<String> {
        let out = match self {
            Pick::Browser => ipc::run("omarchy-default-browser", &[]),
            Pick::Terminal => ipc::run("omarchy-default-terminal", &[]),
            Pick::Editor => ipc::run("omarchy-default-editor", &[]),
            Pick::Font => ipc::run("omarchy-font-current", &[]),
            Pick::TextSize => {
                return text_size(&ipc::run("omarchy-display-text-size", &[]).ok()?);
            }
            Pick::Power => {
                return ipc::run("omarchy-powerprofiles-list", &["--active-state"])
                    .ok()?
                    .lines()
                    .find_map(|l| l.strip_suffix("\t1").map(String::from));
            }
        };
        out.ok().filter(|s| !s.is_empty())
    }

    /// Opciones instaladas: (valor, nombre visible).
    pub fn options(self) -> Vec<(String, String)> {
        match self {
            Pick::Power => lines("omarchy-powerprofiles-list", &[])
                .into_iter()
                .map(|p| {
                    let label = self.label(&p);
                    (p, label)
                })
                .collect(),
            Pick::TextSize => (9..=20)
                .map(|n| {
                    let v = n.to_string();
                    let label = self.label(&v);
                    (v, label)
                })
                .collect(),
            Pick::Font => lines("omarchy-font-list", &[])
                .into_iter()
                .map(|f| (f.clone(), f))
                .collect(),
            _ => self
                .table()
                .iter()
                .filter(|(_, _, c)| installed(c))
                .map(|(v, n, _)| (v.to_string(), n.to_string()))
                .collect(),
        }
    }

    /// Nombre visible de un valor (`zen` → `Zen`).
    pub fn label(self, value: &str) -> String {
        if self == Pick::TextSize {
            return if value == "12" {
                crate::i18n::tf("sys.text_size.normal", &[("n", value)])
            } else {
                format!("{value} px")
            };
        }
        if self == Pick::Power {
            return match value {
                "power-saver" => t("sys.profile.saver"),
                "balanced" => t("sys.profile.balanced"),
                "performance" => t("sys.profile.performance"),
                v => v.to_string(),
            };
        }
        self.table()
            .iter()
            .find(|(v, _, _)| *v == value)
            .map(|(_, n, _)| n.to_string())
            .unwrap_or_else(|| value.to_string())
    }

    /// Cambia el valor con el script de Omarchy.
    pub fn set(self, value: &str) -> Result<(), String> {
        let (cmd, args): (&str, Vec<&str>) = match self {
            Pick::Browser => ("omarchy-default-browser", vec![value]),
            Pick::Terminal => ("omarchy-default-terminal", vec![value]),
            Pick::Editor => ("omarchy-default-editor", vec![value]),
            Pick::Font => ("omarchy-font-set", vec![value]),
            // Shell, apps GTK y terminales a la vez.
            Pick::TextSize => ("omarchy-display-text-size", vec![value]),
            // Lo recuerda para la fuente de energía actual (enchufado o batería).
            Pick::Power => ("omarchy-powerprofiles-set", vec!["autodetect", value]),
        };
        ipc::run(cmd, &args).map(|_| ())
    }
}

/// «Text size: 12 px» (o en minúsculas, según la terminal) → `12`.
fn text_size(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let (key, value) = l.split_once(':')?;
        (key.trim().eq_ignore_ascii_case("text size"))
            .then(|| {
                value
                    .trim()
                    .strip_suffix("px")
                    .map(|v| v.trim().to_string())
            })
            .flatten()
    })
}

fn lines(cmd: &str, args: &[&str]) -> Vec<String> {
    ipc::run(cmd, args)
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn installed(c: &Check) -> bool {
    match c {
        Check::Bin(bins) => bins.iter().any(|b| ipc::command_exists(b)),
        Check::Desktop(id) => desktop_dirs().iter().any(|d| d.join(id).exists()),
    }
}

fn desktop_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    vec![
        home.join(".local/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
    ]
}

/// Paneles del shell de Omarchy: (clave de texto, panel, atajo).
pub const PANELS: [(&str, &str, &str); 4] = [
    ("sys.panel.network", "omarchy.network", "SUPER + CTRL + W"),
    (
        "sys.panel.bluetooth",
        "omarchy.bluetooth",
        "SUPER + CTRL + B",
    ),
    ("sys.panel.audio", "omarchy.audio", "SUPER + CTRL + A"),
    ("sys.panel.power", "omarchy.power", "SUPER + CTRL + P"),
];

/// Abre (o cierra) un panel del shell sin esperar.
pub fn toggle_panel(panel: &str) -> Result<(), String> {
    spawn("omarchy-shell", &["shell", "toggle", panel])
}

/// Programas gráficos que se abren aparte: (clave de texto, programa, argumentos).
pub const GAMING_MENU: (&str, &str, &[&str]) = (
    "games.install",
    "omarchy-menu",
    &["summon", "install.gaming"],
);
pub const PRINTERS_APP: (&str, &str, &[&str]) = ("print.open", "system-config-printer", &[]);

/// Lanza un programa sin esperar a que termine.
pub fn spawn(program: &str, args: &[&str]) -> Result<(), String> {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{program}: {e}"))
}

/// Tareas que se ejecutan en la terminal (pueden pedir la contraseña).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    RestartWifi,
    RestartBluetooth,
    RestartAudio,
    SnapshotCreate,
    SnapshotRestore,
    SnapperSetup,
    EnablePrinting,
}

impl Task {
    pub const REPAIRS: [Task; 3] = [
        Task::RestartWifi,
        Task::RestartBluetooth,
        Task::RestartAudio,
    ];

    /// Clave del texto del botón.
    pub fn label(self) -> &'static str {
        match self {
            Task::RestartWifi => "sys.restart.wifi",
            Task::RestartBluetooth => "sys.restart.bluetooth",
            Task::RestartAudio => "sys.restart.audio",
            Task::SnapshotCreate => "snap.create",
            Task::SnapshotRestore => "snap.keep",
            Task::SnapperSetup => "snap.setup",
            Task::EnablePrinting => "print.enable",
        }
    }

    /// Clave del texto que se confirma antes; `None` si no hace falta.
    pub fn confirm(self) -> Option<&'static str> {
        match self {
            Task::SnapshotRestore => Some("snap.keep.confirm"),
            Task::SnapperSetup => Some("snap.setup.confirm"),
            _ => None,
        }
    }

    /// `omarchy` es la carpeta de Omarchy (para su script de snapper).
    pub fn command(self, omarchy: &Path) -> Command {
        let (program, args): (&str, Vec<String>) = match self {
            Task::RestartWifi => ("omarchy-restart-wifi", vec![]),
            Task::RestartBluetooth => ("omarchy-restart-bluetooth", vec![]),
            Task::RestartAudio => ("omarchy-restart-audio", vec![]),
            Task::EnablePrinting => (
                "sudo",
                ["systemctl", "enable", "--now", "cups.socket"]
                    .map(String::from)
                    .to_vec(),
            ),
            Task::SnapshotCreate => ("omarchy-snapshot", vec!["create".into()]),
            Task::SnapshotRestore => ("omarchy-snapshot", vec!["restore".into()]),
            // Lo mismo que recomienda `omarchy-snapshot` cuando falta.
            Task::SnapperSetup => (
                "sudo",
                vec![
                    "bash".into(),
                    "-euo".into(),
                    "pipefail".into(),
                    omarchy
                        .join("install/config/snapper.sh")
                        .to_string_lossy()
                        .into(),
                ],
            ),
        };
        Command {
            program: program.into(),
            args,
        }
    }
}

/// Puntos de restauración del sistema (snapper).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Snapper {
    #[default]
    Missing,
    /// Instalado pero sin configurar: `omarchy-snapshot` no guarda nada.
    Unconfigured,
    Ready,
}

impl Snapper {
    fn detect() -> Snapper {
        if !ipc::command_exists("snapper") {
            return Snapper::Missing;
        }
        let configured =
            std::fs::read_dir("/etc/snapper/configs").is_ok_and(|mut d| d.next().is_some());
        if configured {
            Snapper::Ready
        } else {
            Snapper::Unconfigured
        }
    }
}

/// Valores en uso, leídos de una vez al entrar en la sección.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub values: HashMap<Pick, String>,
    pub snapper: Snapper,
    /// El sistema arrancó desde un punto de restauración (menú de Limine).
    pub in_snapshot: bool,
    /// El servicio de impresión (CUPS) arranca solo.
    pub printing: Printing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Printing {
    #[default]
    Missing,
    Disabled,
    Ready,
}

impl Printing {
    fn detect() -> Printing {
        if !ipc::command_exists("lpstat")
            && !std::path::Path::new("/usr/lib/systemd/system/cups.socket").exists()
        {
            return Printing::Missing;
        }
        let enabled =
            |unit| ipc::run("systemctl", &["is-enabled", unit]).is_ok_and(|s| s == "enabled");
        if enabled("cups.socket") || enabled("cups.service") {
            Printing::Ready
        } else {
            Printing::Disabled
        }
    }
}

impl State {
    pub fn load() -> State {
        State {
            values: Pick::ALL
                .iter()
                .filter_map(|p| Some((*p, p.current()?)))
                .collect(),
            snapper: Snapper::detect(),
            in_snapshot: std::fs::read_to_string("/proc/cmdline")
                .is_ok_and(|c| booted_from_snapshot(&c)),
            printing: Printing::detect(),
        }
    }
}

/// Como `limine-snapper-restore`: la raíz es `…/<n>/snapshot`.
fn booted_from_snapshot(cmdline: &str) -> bool {
    cmdline.split_whitespace().any(|arg| {
        arg.starts_with("rootflags=")
            && arg.split(',').any(|o| {
                o.strip_prefix("subvol=")
                    .or_else(|| o.strip_prefix("rootflags=subvol="))
                    .and_then(|v| v.strip_suffix("/snapshot"))
                    .and_then(|v| v.rsplit('/').next())
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_known_values() {
        assert_eq!(Pick::Browser.label("zen"), "Zen");
        assert_eq!(Pick::Editor.label("nvim"), "Neovim");
        assert_eq!(
            Pick::Font.label("JetBrainsMono Nerd Font"),
            "JetBrainsMono Nerd Font"
        );
    }

    #[test]
    fn detects_snapshot_boot() {
        assert!(booted_from_snapshot(
            "quiet rootflags=subvol=/@/.snapshots/12/snapshot rw"
        ));
        assert!(booted_from_snapshot(
            "rootflags=compress=zstd,subvol=@/.snapshots/3/snapshot"
        ));
        assert!(!booted_from_snapshot("quiet rootflags=subvol=@ rw"));
    }

    #[test]
    fn reads_text_size() {
        let out = "text size: 12 px\ngtk text-scaling-factor: 1.0\n";
        assert_eq!(text_size(out).as_deref(), Some("12"));
        assert_eq!(text_size("Text size: 16 px").as_deref(), Some("16"));
        assert_eq!(text_size("terminal font: 9 pt"), None);
    }
}
