//! Lo que la tienda pregunta a pacman y los comandos que pide ejecutar.

use std::collections::HashSet;
use std::process::{Command as Proc, Stdio};

use lizarbe_core::term::Command;

use crate::catalogo::Fuente;

/// Paquetes instalados y paquetes que ofrecen los repositorios.
#[derive(Debug, Default, Clone)]
pub struct Estado {
    pub instalados: HashSet<String>,
    pub disponibles: HashSet<String>,
}

impl Estado {
    pub fn cargar() -> Estado {
        Estado {
            instalados: lineas(&salida("pacman", &["-Qq"])),
            disponibles: lineas(&salida("pacman", &["-Slq"])),
        }
    }

    pub fn recargar_instalados(&mut self) {
        self.instalados = lineas(&salida("pacman", &["-Qq"]));
    }

    /// Todos los paquetes están instalados.
    pub fn instalada(&self, paquetes: &[String]) -> bool {
        paquetes.iter().all(|p| self.instalados.contains(p))
    }

    /// Se puede instalar (o ya está). Los del AUR no se comprueban aquí: su lista
    /// es enorme y vive en internet; si uno desapareciera, la instalación avisa.
    pub fn ofrecida(&self, paquetes: &[String], fuente: Fuente) -> bool {
        fuente == Fuente::Aur
            || paquetes
                .iter()
                .all(|p| self.disponibles.contains(p) || self.instalados.contains(p))
    }
}

/// Datos de un paquete según `pacman -Si`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Info {
    pub repo: String,
    pub version: String,
    pub descarga: String,
    pub instalado: String,
}

impl Info {
    /// Del AUR solo se conoce lo que pacman sabe de un paquete ya instalado.
    pub fn consultar(paquete: &str, fuente: Fuente) -> Option<Info> {
        let modo = if fuente == Fuente::Aur { "-Qi" } else { "-Si" };
        let out = salida("pacman", &[modo, paquete]);
        (!out.is_empty()).then(|| Info::parse(&out))
    }

    pub fn parse(text: &str) -> Info {
        let mut i = Info::default();
        for l in text.lines() {
            let Some((k, v)) = l.split_once(':') else {
                continue;
            };
            let v = v.trim().to_string();
            match k.trim() {
                "Repository" => i.repo = v,
                "Version" => i.version = v,
                "Download Size" => i.descarga = v,
                "Installed Size" => i.instalado = v,
                _ => {}
            }
        }
        i
    }
}

fn lineas(text: &str) -> HashSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// Salida estándar de un comando, en inglés para poder leerla; vacía si falla.
fn salida(program: &str, args: &[&str]) -> String {
    Proc::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn cmd(program: &str, args: Vec<String>) -> Command {
    Command {
        program: program.into(),
        args,
    }
}

pub fn instalar(paquetes: &[String], fuente: Fuente) -> Command {
    match fuente {
        Fuente::Repos => {
            let mut a: Vec<String> = ["pacman", "-S", "--needed", "--noconfirm"]
                .map(String::from)
                .into();
            a.extend(paquetes.iter().cloned());
            cmd("sudo", a)
        }
        // yay no se ejecuta como root: pide sudo él mismo cuando lo necesita.
        Fuente::Aur => {
            let mut a: Vec<String> = ["-S", "--needed", "--noconfirm"].map(String::from).into();
            a.extend(paquetes.iter().map(|p| format!("aur/{p}")));
            cmd("yay", a)
        }
    }
}

pub fn quitar(paquetes: &[String]) -> Command {
    let mut a: Vec<String> = ["pacman", "-Rns", "--noconfirm"].map(String::from).into();
    a.extend(paquetes.iter().cloned());
    cmd("sudo", a)
}

/// El buscador de paquetes de siempre (fzf).
pub fn avanzado(fuente: Fuente) -> Command {
    match fuente {
        Fuente::Repos => cmd("omarchy-pkg-install", vec![]),
        Fuente::Aur => cmd("omarchy-pkg-aur-install", vec![]),
    }
}

/// ¿Existe el lanzador `<desktop>.desktop` en alguna carpeta de aplicaciones?
fn lanzador_existe(desktop: &str) -> bool {
    let datos = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|d| !d.is_empty())
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| format!("{h}/.local/share"))
        });
    let sistema = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    datos
        .into_iter()
        .chain(sistema.split(':').map(String::from))
        .any(|d| {
            std::path::Path::new(&d)
                .join("applications")
                .join(format!("{desktop}.desktop"))
                .exists()
        })
}

/// Abre la aplicación sin suspender la tienda.
pub fn abrir(desktop: &str) -> Result<(), String> {
    if !lanzador_existe(desktop) {
        return Err(desktop.to_string());
    }
    Proc::new("uwsm-app")
        .args(["--", "gtk-launch", desktop])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogo::Catalogo;

    #[test]
    fn parses_pacman_si() {
        let txt = "Repository      : extra\nName            : htop\nVersion         : 3.4.1-1\nDownload Size   : 200.50 KiB\nInstalled Size  : 450.00 KiB\n";
        let i = Info::parse(txt);
        assert_eq!(i.repo, "extra");
        assert_eq!(i.version, "3.4.1-1");
        assert_eq!(i.descarga, "200.50 KiB");
        assert_eq!(i.instalado, "450.00 KiB");
    }

    #[test]
    fn builds_commands_without_shell() {
        let c = instalar(&["a".into(), "b".into()], Fuente::Repos);
        assert_eq!(c.program, "sudo");
        assert_eq!(
            c.args,
            ["pacman", "-S", "--needed", "--noconfirm", "a", "b"]
        );
        let c = instalar(&["a".into()], Fuente::Aur);
        assert_eq!(c.program, "yay");
        assert_eq!(c.args, ["-S", "--needed", "--noconfirm", "aur/a"]);
        assert_eq!(quitar(&["a".into()]).args[1], "-Rns");
        assert_eq!(avanzado(Fuente::Repos).program, "omarchy-pkg-install");
        assert_eq!(avanzado(Fuente::Aur).program, "omarchy-pkg-aur-install");
    }

    #[test]
    fn state_logic() {
        let mut e = Estado::default();
        e.disponibles.insert("a".into());
        e.instalados.insert("b".into());
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(e.ofrecida(&v(&["a", "b"]), Fuente::Repos));
        assert!(!e.ofrecida(&v(&["a", "c"]), Fuente::Repos));
        assert!(e.ofrecida(&v(&["c"]), Fuente::Aur));
        assert!(e.instalada(&v(&["b"])));
        assert!(!e.instalada(&v(&["a", "b"])));
    }

    /// Mantenimiento (necesita internet): cada paquete del catálogo debe existir en su fuente.
    #[test]
    #[ignore]
    fn catalog_packages_exist_in_repos() {
        let e = Estado::cargar();
        assert!(!e.disponibles.is_empty(), "pacman -Slq no devolvió nada");
        let cat = Catalogo::embebido();
        let falta: Vec<String> = cat
            .apps
            .iter()
            .filter(|a| a.fuente == Fuente::Repos)
            .flat_map(|a| a.paquetes.iter())
            .filter(|p| !e.disponibles.contains(*p))
            .cloned()
            .collect();
        assert!(falta.is_empty(), "no están en los repos: {falta:?}");
        let falta_aur: Vec<&String> = cat
            .apps
            .iter()
            .filter(|a| a.fuente == Fuente::Aur)
            .flat_map(|a| a.paquetes.iter())
            .filter(|p| salida("yay", &["-Si", &format!("aur/{p}")]).is_empty())
            .collect();
        assert!(falta_aur.is_empty(), "no están en el AUR: {falta_aur:?}");
    }
}
