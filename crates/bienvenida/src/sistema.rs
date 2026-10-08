//! Lo que la Bienvenida lee del sistema y las órdenes que lanza (siempre en
//! segundo plano, para no congelar la animación). En modo demostración
//! (`--demo`) no se ejecuta nada: se responde con datos de ejemplo.

use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

/// Respuestas de las tareas en segundo plano.
#[derive(Debug)]
pub enum Msg {
    Telefonos(Vec<Telefono>),
    Terminal { orden: String, salida: String },
    Tema { nombre: String, ok: bool },
    Fondo,
    Vincular(bool),
    Sonar(bool),
}

/// Primer nombre del usuario (campo GECOS) o su nombre de usuario.
pub fn nombre_usuario() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let gecos = salida("getent", &["passwd", &user])
        .and_then(|l| l.split(':').nth(4).map(str::to_string))
        .map(|g| g.split(',').next().unwrap_or("").trim().to_string())
        .filter(|g| !g.is_empty());
    let nombre = gecos.unwrap_or(user);
    let primero = nombre.split_whitespace().next().unwrap_or("").to_string();
    capitalizar(&primero)
}

fn capitalizar(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Salida estándar de una orden, o `None` si falla.
pub fn salida(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

// ---------------------------------------------------------------- teléfono

#[derive(Debug, Clone, PartialEq)]
pub enum Estado {
    Vinculado,
    Solicitado,
    Nuevo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Telefono {
    pub id: String,
    pub nombre: String,
    pub estado: Estado,
    pub alcanzable: bool,
}

/// Interpreta `kdeconnect-cli -l`: `- Nombre: id (paired and reachable)`.
pub fn parse_telefonos(texto: &str) -> Vec<Telefono> {
    texto
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix("- ")?;
            let (nombre, resto) = l.split_once(": ")?;
            let (id, estado) = resto.split_once(" (")?;
            let estado = estado.trim_end_matches(')');
            let vinculado = estado.contains("paired") && !estado.contains("not paired");
            Some(Telefono {
                id: id.trim().to_string(),
                nombre: nombre.trim().to_string(),
                estado: if vinculado {
                    Estado::Vinculado
                } else if estado.contains("requested") {
                    Estado::Solicitado
                } else {
                    Estado::Nuevo
                },
                alcanzable: estado.contains("reachable") && !estado.contains("unreachable"),
            })
        })
        .collect()
}

pub fn telefonos() -> Vec<Telefono> {
    salida("kdeconnect-cli", &["--refresh", "-l"])
        .or_else(|| salida("kdeconnect-cli", &["-l"]))
        .map(|t| parse_telefonos(&t))
        .unwrap_or_default()
}

pub fn kdeconnect_instalado() -> bool {
    lizarbe_core::ipc::command_exists("kdeconnect-cli")
}

/// ¿El cortafuegos deja pasar KDE Connect? (sin UFW activo, sí).
pub fn cortafuegos_ok() -> bool {
    let activo = salida("systemctl", &["is-active", "ufw"]).is_some_and(|s| s.trim() == "active");
    if !activo {
        return true;
    }
    std::fs::read_to_string("/etc/ufw/user.rules").is_ok_and(|t| t.contains("1714"))
}

// ---------------------------------------------------------------- terminal de juguete

#[derive(Debug, PartialEq)]
pub enum Orden {
    Vacia,
    Ayuda,
    Limpiar,
    Eco(String),
    Ejecutar {
        prog: &'static str,
        args: Vec<String>,
    },
    NoPermitida,
}

/// Las órdenes que acepta la demostración: todas inofensivas.
pub const ORDENES: [&str; 8] = [
    "fastfetch",
    "lizarbe status",
    "date",
    "whoami",
    "uname -a",
    "free -h",
    "ls",
    "help",
];

pub fn interpretar(linea: &str) -> Orden {
    let l = linea.trim();
    if l.is_empty() {
        return Orden::Vacia;
    }
    if let Some(txt) = l.strip_prefix("echo ") {
        return Orden::Eco(txt.trim().to_string());
    }
    let mut it = l.split_whitespace();
    let cmd = it.next().unwrap_or("");
    let args: Vec<&str> = it.collect();
    let ej = |prog: &'static str, a: &[&str]| Orden::Ejecutar {
        prog,
        args: a.iter().map(|s| s.to_string()).collect(),
    };
    match (cmd, args.as_slice()) {
        ("help" | "ayuda", []) => Orden::Ayuda,
        ("clear" | "limpiar", []) => Orden::Limpiar,
        ("fastfetch", []) => ej("fastfetch", &["--pipe", "false"]),
        ("lizarbe", ["status"]) => ej("lizarbe", &["status"]),
        ("date", []) => ej("date", &[]),
        ("whoami", []) => ej("whoami", &[]),
        ("uname", ["-a"]) => ej("uname", &["-a"]),
        ("free", ["-h"]) => ej("free", &["-h"]),
        ("ls", []) => ej("ls", &["--color=always"]),
        _ => Orden::NoPermitida,
    }
}

/// Resultado de ejemplo en modo demostración.
pub fn salida_demo(orden: &str) -> String {
    match orden.trim() {
        "fastfetch" => "\u{1b}[1;31mlizarbe\u{1b}[0m@\u{1b}[1;31mpc\u{1b}[0m\n\u{1b}[1mOS\u{1b}[0m: Omarchy (Arch Linux)\n\u{1b}[1mWM\u{1b}[0m: Hyprland\n\u{1b}[1mMemoria\u{1b}[0m: 612 MiB".into(),
        "lizarbe status" => "Paquetes de Lizarbe:\n  lizarbe  1-2\n  lizarbe-ajustes  1.0.0".into(),
        "date" => "jue 08 oct 2026 12:00:00".into(),
        "whoami" => "lizarbe".into(),
        "uname -a" => "Linux lizarbe 7.2.5 x86_64 GNU/Linux".into(),
        "free -h" => "Mem: 15Gi usada 612Mi libre 14Gi".into(),
        _ => "Documentos  Descargas  Imágenes  Música".into(),
    }
}

// ---------------------------------------------------------------- tareas

/// Ejecuta `f` en un hilo y envía su resultado.
pub fn en_segundo_plano<F>(tx: &Sender<Msg>, f: F)
where
    F: FnOnce() -> Msg + Send + 'static,
{
    let tx = tx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
}

/// Lanza un programa sin esperarlo ni atarlo a la Bienvenida.
pub fn lanzar(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

// ---------------------------------------------------------------- temas

/// Temas instalados, los de Lizarbe primero.
pub fn temas() -> Vec<String> {
    let mut v: Vec<String> = salida("omarchy-theme-list", &[])
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|n| (!n.to_lowercase().starts_with("lizarbe"), n.clone()));
    v
}

pub fn tema_actual() -> Option<String> {
    salida("omarchy-theme-current", &[])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Los tres colores con los que se dibuja la vista previa de un tema.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColoresTema {
    pub fondo: ratatui::style::Color,
    pub texto: ratatui::style::Color,
    pub acento: ratatui::style::Color,
}

/// Lee `background`, `foreground` y `accent` de un `colors.toml`.
pub fn parse_colores_tema(texto: &str) -> Option<ColoresTema> {
    use lizarbe_core::color::parse_hex;
    let valor = |clave: &str| -> Option<ratatui::style::Color> {
        let l = texto.lines().find(|l| {
            let l = l.trim_start();
            l.starts_with(clave) && l[clave.len()..].trim_start().starts_with('=')
        })?;
        let hex = l
            .split('=')
            .nth(1)?
            .trim()
            .trim_matches(|c| c == '"' || c == '\'');
        let (r, g, b) = parse_hex(hex)?;
        Some(ratatui::style::Color::Rgb(r, g, b))
    };
    Some(ColoresTema {
        fondo: valor("background")?,
        texto: valor("foreground")?,
        acento: valor("accent")?,
    })
}

/// Colores de un tema por su nombre («Tokyo Night» → `tokyo-night`), mirando
/// primero los del usuario y luego los de Omarchy.
pub fn colores_tema(nombre: &str) -> Option<ColoresTema> {
    let slug = nombre.trim().to_lowercase().replace(' ', "-");
    let rutas = [
        lizarbe_core::paths::home()
            .join(".config/omarchy/themes")
            .join(&slug),
        lizarbe_core::paths::omarchy().join("themes").join(&slug),
    ];
    rutas
        .iter()
        .find_map(|d| std::fs::read_to_string(d.join("colors.toml")).ok())
        .and_then(|t| parse_colores_tema(&t))
}

// ---------------------------------------------------------------- marca de «pendiente»

/// Archivo que pide mostrar la Bienvenida en el próximo inicio.
pub fn marca_pendiente() -> std::path::PathBuf {
    lizarbe_core::paths::home().join(".config/lizarbe/bienvenida-pendiente")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phones_are_read_from_kdeconnect() {
        let out = "- Galaxy A06: 8f7de10c (paired and reachable)\n- iPhone de Ana: abc123 (reachable)\n- Viejo: zz9 (paired)\n- Pidiendo: p1 (pairing requested)\n- Raro: r2 (not paired)\n1 device found\n";
        let t = parse_telefonos(out);
        assert_eq!(t.len(), 5);
        assert_eq!(
            (t[0].estado.clone(), t[0].alcanzable),
            (Estado::Vinculado, true)
        );
        assert_eq!(t[0].nombre, "Galaxy A06");
        assert_eq!(
            (t[1].estado.clone(), t[1].alcanzable),
            (Estado::Nuevo, true)
        );
        assert_eq!(
            (t[2].estado.clone(), t[2].alcanzable),
            (Estado::Vinculado, false)
        );
        assert_eq!(t[3].estado, Estado::Solicitado);
        assert_eq!(
            (t[4].estado.clone(), t[4].alcanzable),
            (Estado::Nuevo, false)
        );
    }

    #[test]
    fn toy_terminal_only_runs_harmless_commands() {
        assert_eq!(interpretar(""), Orden::Vacia);
        assert_eq!(interpretar("help"), Orden::Ayuda);
        assert_eq!(
            interpretar("echo hola mundo"),
            Orden::Eco("hola mundo".into())
        );
        assert!(matches!(
            interpretar("fastfetch"),
            Orden::Ejecutar {
                prog: "fastfetch",
                ..
            }
        ));
        assert!(matches!(
            interpretar("lizarbe status"),
            Orden::Ejecutar { .. }
        ));
        for peligrosa in [
            "rm -rf /",
            "sudo reboot",
            "lizarbe update",
            "ls /etc",
            "cat /etc/passwd",
            "fastfetch; rm x",
        ] {
            assert_eq!(interpretar(peligrosa), Orden::NoPermitida, "{peligrosa}");
        }
        // Todas las que se sugieren se aceptan.
        for o in ORDENES {
            assert_ne!(interpretar(o), Orden::NoPermitida, "{o}");
        }
    }

    #[test]
    fn theme_colors_are_read_from_colors_toml() {
        use ratatui::style::Color;
        let t = "accent = \"#B3171E\"\nbackground = \"#D8C79E\"\nforeground = '#1E1B16'\nmuted = \"#5F5440\"\n";
        let c = parse_colores_tema(t).unwrap();
        assert_eq!(c.acento, Color::Rgb(0xB3, 0x17, 0x1E));
        assert_eq!(c.fondo, Color::Rgb(0xD8, 0xC7, 0x9E));
        assert_eq!(c.texto, Color::Rgb(0x1E, 0x1B, 0x16));
        assert!(parse_colores_tema("accent = \"#fff\"").is_none());
    }

    #[test]
    fn names_are_capitalized() {
        assert_eq!(capitalizar("leonardo"), "Leonardo");
        assert_eq!(capitalizar(""), "");
    }
}
