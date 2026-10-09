//! KDE Connect: los teléfonos que ve `kdeconnect-cli` y la carpeta donde
//! recibe los archivos.
//!
//! Se le pregunta al demonio por D-Bus (`busctl`): responde al instante y no
//! depende del idioma. `kdeconnect-cli` queda de respaldo; traduce su salida
//! al idioma del sistema y espera dos segundos en cada llamada, así que se
//! ejecuta con `LANGUAGE=en` para leer siempre las mismas palabras.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

impl Telefono {
    /// Vinculado y a la vista: se le pueden mandar y pedir cosas.
    pub fn conectado(&self) -> bool {
        self.estado == Estado::Vinculado && self.alcanzable
    }
}

/// Interpreta `kdeconnect-cli -l` en inglés:
/// `- Nombre: id on 192.168.1.39 via LAN (paired and reachable)`
/// (las versiones antiguas no traen el `on … via …`).
pub fn parse_telefonos(texto: &str) -> Vec<Telefono> {
    texto
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix("- ")?;
            let (resto, estado) = l.rsplit_once(" (")?;
            let estado = estado.trim_end_matches(')');
            // El nombre puede llevar «: »; el id va tras el último.
            let (nombre, resto) = resto.rsplit_once(": ")?;
            let id = resto.split_whitespace().next()?;
            let vinculado = estado.contains("paired") && !estado.contains("not paired");
            Some(Telefono {
                id: id.to_string(),
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

fn cli(args: &[&str]) -> Option<String> {
    let out = Command::new("kdeconnect-cli")
        .args(args)
        .env("LANGUAGE", "en")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn instalado() -> bool {
    crate::ipc::command_exists("kdeconnect-cli")
}

const DEST: &str = "org.kde.kdeconnect";

/// Llama a un método de KDE Connect por D-Bus y devuelve su respuesta en JSON.
fn busctl(path: &str, interface: &str, metodo: &str, firma: &[&str]) -> Option<serde_json::Value> {
    let out = Command::new("busctl")
        .args([
            "--user",
            "--json=short",
            "call",
            DEST,
            path,
            interface,
            metodo,
        ])
        .args(firma)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| serde_json::from_slice(&out.stdout).ok())?
}

fn propiedad<'a>(props: &'a serde_json::Value, nombre: &str) -> Option<&'a serde_json::Value> {
    props.get(nombre)?.get("data")
}

/// Teléfonos según el demonio de KDE Connect.
fn telefonos_dbus() -> Option<Vec<Telefono>> {
    let ids = busctl(
        "/modules/kdeconnect",
        "org.kde.kdeconnect.daemon",
        "devices",
        &["bb", "false", "false"],
    )?;
    let ids: Vec<&str> = ids
        .get("data")?
        .get(0)?
        .as_array()?
        .iter()
        .filter_map(|i| i.as_str())
        .collect();
    Some(
        ids.into_iter()
            .filter_map(|id| {
                let r = busctl(
                    &format!("/modules/kdeconnect/devices/{id}"),
                    "org.freedesktop.DBus.Properties",
                    "GetAll",
                    &["s", "org.kde.kdeconnect.device"],
                )?;
                let props = r.get("data")?.get(0)?;
                let es = |n: &str| {
                    propiedad(props, n)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                };
                Some(Telefono {
                    id: id.to_string(),
                    nombre: propiedad(props, "name")?.as_str()?.to_string(),
                    estado: if es("isPaired") {
                        Estado::Vinculado
                    } else if es("isPairRequested") || es("isPairRequestedByPeer") {
                        Estado::Solicitado
                    } else {
                        Estado::Nuevo
                    },
                    alcanzable: es("isReachable"),
                })
            })
            .collect(),
    )
}

/// Teléfonos conocidos; `None` si KDE Connect no responde. Con `refrescar`
/// se le pide antes al demonio que vuelva a buscar en la red.
pub fn telefonos(refrescar: bool) -> Option<Vec<Telefono>> {
    if refrescar {
        busctl(
            "/modules/kdeconnect",
            "org.kde.kdeconnect.daemon",
            "forceOnNetworkChange",
            &[],
        );
    }
    telefonos_dbus().or_else(|| {
        let texto = if refrescar {
            cli(&["--refresh", "-l"]).or_else(|| cli(&["-l"]))
        } else {
            cli(&["-l"])
        }?;
        Some(parse_telefonos(&texto))
    })
}

/// Manda un aviso al teléfono `id`; `true` si KDE Connect lo aceptó.
pub fn avisar(id: &str, mensaje: &str) -> bool {
    busctl(
        &format!("/modules/kdeconnect/devices/{id}/ping"),
        "org.kde.kdeconnect.device.ping",
        "sendPing",
        &["s", mensaje],
    )
    .is_some()
        || cli(&["-d", id, "--ping-msg", mensaje]).is_some()
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::paths::home().join(".config"))
        .join("kdeconnect")
}

/// Nombre de este equipo en KDE Connect (el que ve el teléfono); si no
/// está configurado, el del equipo.
pub fn nombre_equipo() -> String {
    std::fs::read_to_string(config_dir().join("config"))
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.trim().strip_prefix("name=").map(|v| v.trim().to_string()))
        })
        .filter(|n| !n.is_empty())
        .or_else(|| {
            let h = std::fs::read_to_string("/proc/sys/kernel/hostname").ok()?;
            Some(h.trim().to_string()).filter(|h| !h.is_empty())
        })
        .unwrap_or_else(|| "lizarbe".to_string())
}

/// Lee `incoming_path` de la configuración del complemento de compartir.
fn carpeta_configurada(config: &str, home: &Path) -> Option<PathBuf> {
    let valor = config
        .lines()
        .find_map(|l| l.trim().strip_prefix("incoming_path="))?
        .trim()
        .trim_matches('"');
    let valor = valor
        .strip_prefix("$HOME")
        .or_else(|| valor.strip_prefix('~'))
        .map(|r| format!("{}{r}", home.display()))
        .unwrap_or_else(|| valor.to_string());
    (!valor.is_empty()).then(|| PathBuf::from(valor))
}

/// Carpeta donde llegan los archivos del teléfono `id`: la que configuró
/// en KDE Connect o, si no, la de descargas del usuario.
pub fn carpeta_destino(id: Option<&str>) -> PathBuf {
    let home = crate::paths::home();
    id.and_then(|id| {
        let f = config_dir().join(id).join("kdeconnect_share/config");
        carpeta_configurada(&std::fs::read_to_string(f).ok()?, &home)
    })
    .or_else(|| {
        let d = crate::ipc::run("xdg-user-dir", &["DOWNLOAD"]).ok()?;
        (!d.is_empty() && Path::new(&d) != home).then(|| PathBuf::from(d))
    })
    .unwrap_or_else(|| home.join("Downloads"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phones_are_read_from_kdeconnect() {
        let out = "- Galaxy A06: 8f7de10c on 192.168.1.39 via LAN (paired and reachable)\n- iPhone de Ana: abc123 (reachable)\n- Viejo: zz9 (paired)\n- Pidiendo: p1 (pairing requested)\n- Raro: r2 (not paired)\n1 device found\n";
        let t = parse_telefonos(out);
        assert_eq!(t.len(), 5);
        assert_eq!(t[0].id, "8f7de10c");
        assert_eq!(t[0].nombre, "Galaxy A06");
        assert!(t[0].conectado());
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
    fn names_with_colons_keep_their_id() {
        let t = parse_telefonos("- Casa: sala: ab12 on 10.0.0.2 via LAN (paired and reachable)\n");
        assert_eq!(
            (t[0].nombre.as_str(), t[0].id.as_str()),
            ("Casa: sala", "ab12")
        );
    }

    #[test]
    fn spanish_output_is_not_understood_but_never_invents_phones() {
        // Por eso se fuerza LANGUAGE=en: sin él no habría "paired".
        let t = parse_telefonos("- Galaxy: ab12 en 10.0.0.2 vía LAN (vinculado y accesible)\n");
        assert!(t.iter().all(|p| !p.conectado()));
    }

    /// Lo que responde el demonio en este equipo (con un teléfono vinculado).
    #[test]
    fn the_daemon_is_asked_without_depending_on_the_language() {
        if !instalado()
            || busctl(
                "/modules/kdeconnect",
                "org.kde.kdeconnect.daemon",
                "devices",
                &["bb", "false", "false"],
            )
            .is_none()
        {
            return; // sin KDE Connect en marcha no hay nada que comprobar
        }
        let lista = telefonos(false).expect("el demonio responde");
        assert!(
            lista
                .iter()
                .all(|p| !p.id.is_empty() && !p.nombre.is_empty())
        );
    }

    #[test]
    fn configured_folder_is_read() {
        let home = Path::new("/home/u");
        assert_eq!(
            carpeta_configurada("[General]\nincoming_path=$HOME/Recibidos\n", home),
            Some(PathBuf::from("/home/u/Recibidos"))
        );
        assert_eq!(
            carpeta_configurada("incoming_path=/srv/in", home),
            Some(PathBuf::from("/srv/in"))
        );
        assert_eq!(carpeta_configurada("[General]\n", home), None);
    }
}
