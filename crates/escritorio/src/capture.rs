//! Capturas de pantalla (`Impr Pant`): lo que Omarchy deja configurar.
//!
//! Omarchy no tiene archivo de ajustes para las capturas: el script
//! `omarchy-capture-screenshot` recibe el modo y el procesado como argumentos
//! (y `--editor=`), y lee la carpeta de las variables de entorno
//! `OMARCHY_SCREENSHOT_DIR` / `OMARCHY_SCREENRECORD_DIR`. Escritorio guarda los
//! valores y los lleva a tres sitios:
//!
//! * el comando del atajo (`escritorio.lua`, ver `hyprfile`);
//! * `hl.env` en ese mismo archivo, para que el atajo los vea ya;
//! * un bloque gestionado en `~/.config/uwsm/env`, para el resto de la sesión
//!   (el menú Capturar de Omarchy) a partir del próximo inicio de sesión.

use std::path::PathBuf;

use serde_json::Value;

use crate::hyprfile::Values;

pub const KEY_DIR: &str = "x:cap_dir";
pub const KEY_MODE: &str = "x:cap_mode";
pub const KEY_PROC: &str = "x:cap_proc";
pub const KEY_EDITOR: &str = "x:cap_editor";
pub const KEY_RECDIR: &str = "x:cap_recdir";

pub const DEFAULT_KEYS: &str = "PRINT";
pub const DEFAULT_MODE: &str = "smart";
pub const DEFAULT_PROC: &str = "slurp";
pub const DEFAULT_EDITOR: &str = "tensaku-edit";

const BEGIN: &str = "# >>> lizarbe capturas (gestionado)";
const END: &str = "# <<< lizarbe capturas";

pub const MODES: [&str; 4] = ["smart", "region", "windows", "fullscreen"];
pub const PROCS: [&str; 3] = ["slurp", "copy", "save"];

/// Carpeta que usa Omarchy si no se fija ninguna.
pub fn default_dir() -> String {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/Pictures".into())
}

pub fn default_recdir() -> String {
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Videos")))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/Videos".into())
}

fn text<'a>(values: &'a Values, key: &str) -> Option<&'a str> {
    values
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// `~` y `~/…` → ruta completa.
pub fn expand(path: &str) -> String {
    let home = || dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == "~" {
        home().display().to_string()
    } else if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest).display().to_string()
    } else {
        path.to_string()
    }
}

/// Comando del atajo si algo difiere de lo que hace Omarchy por defecto.
pub fn command(values: &Values) -> Option<String> {
    let mode = text(values, KEY_MODE).filter(|m| MODES.contains(m));
    let proc = text(values, KEY_PROC).filter(|p| PROCS.contains(p));
    let editor = text(values, KEY_EDITOR).filter(|e| !e.contains(char::is_whitespace));
    if mode.is_none() && proc.is_none() && editor.is_none() {
        return None;
    }
    let mut cmd = format!(
        "omarchy-capture-screenshot {} {}",
        mode.unwrap_or(DEFAULT_MODE),
        proc.unwrap_or(DEFAULT_PROC)
    );
    if let Some(e) = editor {
        cmd.push_str(" --editor=");
        cmd.push_str(e);
    }
    Some(cmd)
}

/// Variables de entorno que corresponden a los valores guardados.
pub fn env_vars(values: &Values) -> Vec<(&'static str, String)> {
    let mut out = vec![];
    if let Some(d) = text(values, KEY_DIR) {
        out.push(("OMARCHY_SCREENSHOT_DIR", expand(d)));
    }
    if let Some(d) = text(values, KEY_RECDIR) {
        out.push(("OMARCHY_SCREENRECORD_DIR", expand(d)));
    }
    if let Some(e) = text(values, KEY_EDITOR) {
        out.push(("OMARCHY_SCREENSHOT_EDITOR", e.to_string()));
    }
    out
}

fn shell_quote(v: &str) -> String {
    let mut s = String::from("\"");
    for c in v.chars() {
        if matches!(c, '"' | '\\' | '$' | '`') {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

/// Pone (o quita) el bloque gestionado de `uwsm/env` sin tocar el resto.
pub fn render_env(existing: &str, vars: &[(&str, String)]) -> String {
    let mut kept: Vec<&str> = vec![];
    let mut inside = false;
    for line in existing.lines() {
        if line.trim() == BEGIN {
            inside = true;
        } else if inside && line.trim() == END {
            inside = false;
        } else if !inside {
            kept.push(line);
        }
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut out = kept.join("\n");
    if !vars.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(BEGIN);
        out.push('\n');
        for (k, v) in vars {
            out.push_str(&format!("export {k}={}\n", shell_quote(v)));
        }
        out.push_str(END);
        out.push('\n');
    } else if !out.is_empty() {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vals(pairs: &[(&str, &str)]) -> Values {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect()
    }

    #[test]
    fn defaults_write_nothing() {
        assert_eq!(command(&Values::new()), None);
        assert!(env_vars(&Values::new()).is_empty());
    }

    #[test]
    fn command_combines_mode_processing_and_editor() {
        let c = |p: &[(&str, &str)]| command(&vals(p));
        assert_eq!(
            c(&[(KEY_MODE, "region")]).unwrap(),
            "omarchy-capture-screenshot region slurp"
        );
        assert_eq!(
            c(&[(KEY_PROC, "copy")]).unwrap(),
            "omarchy-capture-screenshot smart copy"
        );
        assert_eq!(
            c(&[
                (KEY_MODE, "fullscreen"),
                (KEY_PROC, "save"),
                (KEY_EDITOR, "satty")
            ])
            .unwrap(),
            "omarchy-capture-screenshot fullscreen save --editor=satty"
        );
        // valores raros se ignoran
        assert_eq!(c(&[(KEY_MODE, "x; rm -rf /")]), None);
        assert_eq!(c(&[(KEY_EDITOR, "dos palabras")]), None);
    }

    #[test]
    fn env_expands_the_home_folder() {
        let v = env_vars(&vals(&[(KEY_DIR, "~/Capturas"), (KEY_EDITOR, "satty")]));
        let home = dirs::home_dir().unwrap().join("Capturas");
        assert_eq!(v[0], ("OMARCHY_SCREENSHOT_DIR", home.display().to_string()));
        assert_eq!(v[1], ("OMARCHY_SCREENSHOT_EDITOR", "satty".to_string()));
    }

    #[test]
    fn env_block_is_added_replaced_and_removed_keeping_the_rest() {
        let base = "# mis cosas\nexport FOO=1\n";
        let one = render_env(base, &[("OMARCHY_SCREENSHOT_DIR", "/a b/c".into())]);
        assert!(one.starts_with("# mis cosas\nexport FOO=1\n\n# >>> lizarbe capturas"));
        assert!(one.contains("export OMARCHY_SCREENSHOT_DIR=\"/a b/c\"\n"));
        let two = render_env(&one, &[("OMARCHY_SCREENSHOT_DIR", "/otra".into())]);
        assert_eq!(two.matches(BEGIN).count(), 1);
        assert!(two.contains("\"/otra\"") && !two.contains("/a b/c"));
        assert_eq!(render_env(&two, &[]), base);
        // sin nada que escribir ni archivo previo
        assert_eq!(render_env("", &[]), "");
        // comillas y $ se escapan
        let q = render_env("", &[("X", "a\"$b".into())]);
        assert!(q.contains("export X=\"a\\\"\\$b\""));
    }
}
