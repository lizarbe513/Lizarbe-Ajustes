//! Comunicación con Hyprland mediante `hyprctl`: leer el valor efectivo de
//! una opción, vista previa en vivo con `eval`, recarga con comprobación de
//! errores, monitores conectados y toggles de Omarchy.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::ipc;

/// `true` si hay una sesión de Hyprland a la que hablar.
pub fn available() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() && ipc::command_exists("hyprctl")
}

/// Valor de una opción tal como lo devuelve `hyprctl getoption -j`.
#[derive(Debug, Clone, PartialEq)]
pub enum OptValue {
    Int(i64),
    Float(f64),
    Str(String),
    /// Valores compuestos que Hyprland da como texto (p. ej. `"5 5 5 5"`).
    Css(String),
    Other(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HyprOption {
    /// Si alguien la fijó en la configuración (si no, es el valor de fábrica).
    pub set: bool,
    pub value: OptValue,
}

pub fn parse_option(json: &str) -> Result<HyprOption, String> {
    let v: Value = serde_json::from_str(json).map_err(|_| json.trim().to_string())?;
    let obj = v.as_object().ok_or_else(|| json.trim().to_string())?;
    let set = obj.get("set").and_then(Value::as_bool).unwrap_or(false);
    let value = if let Some(i) = obj.get("int").and_then(Value::as_i64) {
        OptValue::Int(i)
    } else if let Some(f) = obj.get("float").and_then(Value::as_f64) {
        OptValue::Float(f)
    } else if let Some(s) = obj.get("str").and_then(Value::as_str) {
        OptValue::Str(s.to_string())
    } else if let Some(s) = obj.get("css").and_then(Value::as_str) {
        OptValue::Css(s.to_string())
    } else {
        let mut rest = obj.clone();
        rest.remove("option");
        rest.remove("set");
        OptValue::Other(Value::Object(rest))
    };
    Ok(HyprOption { set, value })
}

/// Valor efectivo de `name` (p. ej. `general:gaps_in`) después de aplicar
/// toda la configuración: defaults de Omarchy, archivos del usuario y toggles.
pub fn getoption(name: &str) -> Result<HyprOption, String> {
    parse_option(&ipc::run("hyprctl", &["getoption", name, "-j"])?)
}

/// Ejecuta Lua en el Hyprland en marcha (vista previa sin tocar archivos).
pub fn eval(lua: &str) -> Result<(), String> {
    ipc::run("hyprctl", &["eval", lua]).map(|_| ())
}

pub fn reload() -> Result<(), String> {
    ipc::run("hyprctl", &["reload"]).map(|_| ())
}

/// Errores de `hyprctl configerrors -j` (que devuelve `[""]` si no hay).
pub fn parse_config_errors(json: &str) -> Result<Vec<String>, String> {
    let list: Vec<String> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .collect())
}

pub fn config_errors() -> Result<Vec<String>, String> {
    parse_config_errors(&ipc::run("hyprctl", &["configerrors", "-j"])?)
}

/// Recarga la configuración y devuelve los errores que informe Hyprland.
/// Quien escribió los archivos decide si deshace los cambios.
pub fn reload_checked() -> Result<(), Vec<String>> {
    reload().map_err(|e| vec![e])?;
    match config_errors() {
        Ok(errors) if errors.is_empty() => Ok(()),
        Ok(errors) => Err(errors),
        Err(e) => Err(vec![e]),
    }
}

/// Monitor según `hyprctl monitors -j`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f64,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    #[serde(default)]
    pub transform: u8,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub vrr: bool,
    /// `"none"` o el nombre del monitor que se refleja.
    #[serde(default)]
    pub mirror_of: String,
    /// Modos como `"1920x1080@60.00Hz"`.
    #[serde(default)]
    pub available_modes: Vec<String>,
}

pub fn parse_monitors(json: &str) -> Result<Vec<Monitor>, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Monitores conectados; con `all` también los desactivados.
pub fn monitors(all: bool) -> Result<Vec<Monitor>, String> {
    let args: &[&str] = if all {
        &["monitors", "all", "-j"]
    } else {
        &["monitors", "-j"]
    };
    parse_monitors(&ipc::run("hyprctl", args)?)
}

fn toggles_dir() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".local/state/omarchy/toggles/hypr"))
}

/// Equivale a `omarchy-hyprland-toggle-enabled <flag>`. Un toggle activo
/// se carga después de los archivos del usuario y puede pisar sus valores
/// (p. ej. `window-no-gaps` deja espacios y bordes en cero).
pub fn toggle_enabled(flag: &str) -> bool {
    toggles_dir().is_some_and(|d| d.join(format!("{flag}.lua")).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_option_kinds() {
        let o = parse_option(r#"{"option": "decoration:rounding", "int": 0, "set": true }"#)
            .unwrap();
        assert_eq!(o.value, OptValue::Int(0));
        assert!(o.set);
        let o = parse_option(r#"{"option": "general:gaps_in", "css": "5 5 5 5", "set": true }"#)
            .unwrap();
        assert_eq!(o.value, OptValue::Css("5 5 5 5".into()));
        let o = parse_option(r#"{"option": "input:kb_options", "str": "compose:rwin", "set": false }"#)
            .unwrap();
        assert_eq!(o.value, OptValue::Str("compose:rwin".into()));
        assert!(!o.set);
        let o = parse_option(r#"{"option": "x", "float": 0.5, "set": true }"#).unwrap();
        assert_eq!(o.value, OptValue::Float(0.5));
    }

    #[test]
    fn unknown_option_is_an_error() {
        assert_eq!(parse_option("no such option\n"), Err("no such option".into()));
    }

    #[test]
    fn empty_config_errors_mean_none() {
        assert!(parse_config_errors("[\n\t\"\"\n]").unwrap().is_empty());
        assert_eq!(
            parse_config_errors(r#"["error en línea 3", ""]"#).unwrap(),
            vec!["error en línea 3".to_string()]
        );
    }

    #[test]
    fn parses_monitors() {
        let json = r#"[{"id":0,"name":"HDMI-A-1","description":"Samsung",
            "width":1920,"height":1080,"refreshRate":60.0,"x":0,"y":0,"scale":1,
            "transform":0,"focused":true,"disabled":false,"vrr":false,
            "mirrorOf":"none","availableModes":["1920x1080@60.00Hz","1920x1080@75.00Hz"]}]"#;
        let m = parse_monitors(json).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "HDMI-A-1");
        assert_eq!(m[0].scale, 1.0);
        assert_eq!(m[0].mirror_of, "none");
        assert_eq!(m[0].available_modes.len(), 2);
    }
}
