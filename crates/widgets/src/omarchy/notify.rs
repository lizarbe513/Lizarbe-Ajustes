//! Notificaciones del sistema.
//!
//! Omarchy muestra las notificaciones con su propio shell (Quickshell). Si
//! además hay otro programa que las puede mostrar (swaync, que trae
//! `nwg-shell`), gana el que arranque primero y, según la carrera, salen con
//! un estilo u otro. Aquí se leen y escriben las dos cosas que se pueden
//! configurar sin tocar archivos de Omarchy:
//!
//! * **No molestar**, en `~/.local/state/omarchy/notifications.json`.
//! * **Si swaync puede arrancar**: se impide con un archivo de servicio D-Bus
//!   del usuario (que tapa al del sistema) y enmascarando su servicio.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::paths::Paths;
use crate::omarchy::ipc;

/// Nombre D-Bus de las notificaciones.
pub const DBUS_NAME: &str = "org.freedesktop.Notifications";
/// Servicio de usuario de swaync.
pub const SWAYNC_UNIT: &str = "swaync.service";
const MARK: &str = "# Gestionado por Lizarbe (Widgets › Notificaciones). Impide que swaync\n\
                    # tome las notificaciones; para volver, enciende «Dejar que swaync muestre avisos».\n";

/// Lo que se puede configurar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notif {
    /// No molestar activo.
    pub dnd: bool,
    /// swaync puede arrancar (no está bloqueado).
    pub swaync: bool,
}

/// `~/.local/state/omarchy/notifications.json` (en pruebas, en la carpeta de pruebas).
pub fn settings_file(paths: &Paths) -> PathBuf {
    paths.notifications_json()
}

/// El archivo de servicio D-Bus de usuario que bloquea a swaync.
pub fn override_file(paths: &Paths) -> PathBuf {
    paths.dbus_override()
}

pub fn override_content() -> String {
    format!("{MARK}[D-BUS Service]\nName={DBUS_NAME}\nExec=/usr/bin/false\n")
}

/// ¿Es nuestro el archivo? (así no se borra uno que el usuario puso a mano).
fn is_ours(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|t| t.starts_with("# Gestionado por Lizarbe"))
}

/// Lee el estado actual.
pub fn load(paths: &Paths) -> Notif {
    let dnd = std::fs::read_to_string(settings_file(paths))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("dnd").and_then(Value::as_bool))
        .unwrap_or(false);
    Notif {
        dnd,
        swaync: !override_file(paths).exists(),
    }
}

/// Contenido nuevo de `notifications.json` con `dnd`, conservando lo demás.
pub fn render_settings(existing: Option<&str>, dnd: bool) -> String {
    let mut v = existing
        .and_then(|t| serde_json::from_str::<Value>(t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({ "version": 3 }));
    v["dnd"] = json!(dnd);
    let mut out = serde_json::to_string_pretty(&v).unwrap_or_default();
    out.push('\n');
    out
}

/// Programa que tiene ahora las notificaciones (`quickshell`, `swaync`…).
pub fn owner() -> Option<String> {
    if !ipc::command_exists("busctl") {
        return None;
    }
    let out = ipc::run("busctl", &["--user", "status", DBUS_NAME]).ok()?;
    out.lines()
        .find_map(|l| l.strip_prefix("Comm="))
        .map(|s| s.trim().to_string())
}

/// Escribe el archivo de servicio que bloquea a swaync (o lo quita).
pub fn write_override(paths: &Paths, block: bool) -> Result<()> {
    let file = override_file(paths);
    if block {
        lizarbe_core::fsutil::write_atomic(&file, &override_content())
            .with_context(|| format!("{}", file.display()))?;
    } else if is_ours(&file) {
        std::fs::remove_file(&file).with_context(|| format!("{}", file.display()))?;
    }
    Ok(())
}

/// Enmascara (o no) el servicio de swaync y lo detiene si se bloquea.
/// No hace nada en el modo de pruebas.
pub fn set_unit(paths: &Paths, block: bool) -> Vec<String> {
    let mut errors = vec![];
    if paths.sandbox || !ipc::command_exists("systemctl") {
        return errors;
    }
    let steps: &[&[&str]] = if block {
        &[
            &["--user", "mask", SWAYNC_UNIT],
            &["--user", "stop", SWAYNC_UNIT],
        ]
    } else {
        &[&["--user", "unmask", SWAYNC_UNIT]]
    };
    for args in steps {
        if let Err(e) = ipc::run("systemctl", args) {
            errors.push(e);
        }
    }
    errors
}

/// Pone No molestar en el estado pedido en el shell en ejecución (sin
/// reiniciarlo). Devuelve el error si el shell no responde.
pub fn apply_dnd(dnd: bool) -> Result<(), String> {
    // El shell guarda el cambio él mismo; solo se alterna si no coincide.
    let now = ipc::run("omarchy-shell", &["notifications", "toggleDnd"]);
    match now {
        Ok(state) => {
            // `toggleDnd` devuelve el estado nuevo; si ya era el pedido, se deshace.
            let on = matches!(state.trim(), "true" | "on" | "1" | "dnd");
            if on != dnd {
                ipc::run("omarchy-shell", &["notifications", "toggleDnd"]).map(|_| ())
            } else {
                Ok(())
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_keep_other_keys_and_set_dnd() {
        let out = render_settings(Some(r#"{"version":3,"dnd":false,"x":1}"#), true);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, json!({"version": 3, "dnd": true, "x": 1}));
        // Sin archivo, o dañado: uno nuevo y válido.
        for existing in [None, Some("no es json"), Some("[1,2]")] {
            let v: Value = serde_json::from_str(&render_settings(existing, false)).unwrap();
            assert_eq!(v, json!({"version": 3, "dnd": false}));
        }
    }

    #[test]
    fn override_is_a_valid_dbus_service_file() {
        let c = override_content();
        assert!(c.starts_with("# Gestionado por Lizarbe"));
        assert!(c.contains("[D-BUS Service]\nName=org.freedesktop.Notifications\nExec="));
    }

    #[test]
    fn override_is_written_and_removed_only_if_ours() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::detect(Some(dir.path().to_path_buf()));
        assert!(load(&paths).swaync);
        write_override(&paths, true).unwrap();
        assert!(!load(&paths).swaync);
        assert!(is_ours(&override_file(&paths)));
        write_override(&paths, false).unwrap();
        assert!(load(&paths).swaync);
        // Un archivo del usuario no se toca.
        let f = override_file(&paths);
        std::fs::write(&f, "[D-BUS Service]\nName=x\n").unwrap();
        write_override(&paths, false).unwrap();
        assert!(f.exists());
    }

    #[test]
    fn dnd_is_read_from_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::detect(Some(dir.path().to_path_buf()));
        assert!(!load(&paths).dnd);
        std::fs::write(settings_file(&paths), render_settings(None, true)).unwrap();
        assert!(load(&paths).dnd);
    }
}
