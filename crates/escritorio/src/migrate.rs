//! Migración desde Meca HyprConfig: lo que Meca guardó en
//! `hyprland-gui.lua` y en los atajos de `bindings.lua` pasa a
//! `escritorio.lua`, guardando solo lo que difiere de Omarchy.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::catalog::{self, Ctx};
use crate::hyprfile::{CustomBind, Values};

/// Opciones de teclado que pone Omarchy por defecto.
const OMARCHY_KB_OPTIONS: &str = ",shift:both_capslock_cancel";

/// Valor Lua simple: booleano, número o texto.
fn lua_scalar(v: &str) -> Option<Value> {
    let v = v.trim().trim_end_matches(',').trim();
    match v {
        "true" => Some(json!(true)),
        "false" => Some(json!(false)),
        _ => {
            if let Some(s) = v.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                return Some(json!(s));
            }
            let n: f64 = v.parse().ok()?;
            if n.fract() == 0.0 && !v.contains('.') {
                Some(json!(n as i64))
            } else {
                Some(json!(n))
            }
        }
    }
}

/// Valores del bloque `hl.config({ … })` como claves planas `a:b:c`.
/// El formato lo genera Meca: una asignación o una tabla por línea.
pub fn parse_config(text: &str) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    let mut path: Vec<String> = vec![];
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        if l == "hl.config({" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if l == "})" {
            break;
        }
        if l == "}," || l == "}" {
            path.pop();
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        if v == "{" {
            path.push(k.to_string());
        } else if let Some(val) = lua_scalar(v) {
            let mut full = path.clone();
            full.push(k.to_string());
            out.insert(full.join(":"), val);
        }
    }
    out
}

/// Datos de la línea `-- MECA_META: {…}`.
pub fn parse_meta(text: &str) -> serde_json::Map<String, Value> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("-- MECA_META:"))
        .and_then(|j| serde_json::from_str::<Value>(j.trim()).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn close(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => a == b,
    }
}

/// Ajustes de Escritorio equivalentes a lo que Meca tenía guardado.
pub fn settings_from_gui(text: &str) -> Values {
    let ctx = Ctx::default();
    let flat = parse_config(text);
    let meta = parse_meta(text);
    let mut out = Values::new();

    // Nombre normalizado (guion = guion bajo) → clave real del catálogo.
    let keys: HashMap<String, String> = catalog::hypr_keys()
        .into_iter()
        .map(|k| (k.replace('-', "_"), k))
        .collect();
    for (flat_key, v) in &flat {
        let Some(key) = keys.get(flat_key) else {
            continue;
        };
        let Some(def) = catalog::find(key, &ctx) else {
            continue;
        };
        let v = catalog::coerce(&def.kind, v.clone());
        if key == "input:kb_options" {
            if v.as_str() != Some(OMARCHY_KB_OPTIONS) {
                out.insert(key.clone(), v);
            }
        } else if def.default.as_ref().is_none_or(|d| !close(d, &v)) {
            out.insert(key.clone(), v);
        }
    }
    // Las opciones de teclado no están en `hypr_keys` con su nombre normal.
    if let Some(Value::String(opts)) = flat.get("input:kb_options")
        && opts != OMARCHY_KB_OPTIONS
    {
        out.insert("input:kb_options".into(), json!(opts));
    }

    let get = |k: &str| meta.get(k);
    if let Some(s) = get("anim_windows").and_then(Value::as_str)
        && s != "popin 87%"
    {
        out.insert("x:anim_windows".into(), json!(s));
    }
    if get("anim_workspaces_enabled").and_then(Value::as_bool) == Some(true)
        && let Some(s) = get("anim_workspaces").and_then(Value::as_str)
    {
        out.insert("x:anim_workspaces".into(), json!(s));
    }
    match get("animation_preset").and_then(Value::as_str) {
        Some("snappy") | Some("minimal") => {
            out.insert("x:anim_speed".into(), json!("fast"));
        }
        Some("smooth") => {
            out.insert("x:anim_speed".into(), json!("slow"));
        }
        _ => {}
    }
    if let Some(n) = get("workspace_count").and_then(Value::as_i64)
        && n > 0
    {
        out.insert("x:ws_persistent".into(), json!(n));
    }
    if get("workspace_swipe").and_then(Value::as_bool) == Some(true) {
        out.insert("x:ws_swipe".into(), json!(true));
    }
    if let Some(t) = get("cursor_theme").and_then(Value::as_str)
        && !t.is_empty()
        && t != "default"
    {
        out.insert("x:cursor_theme".into(), json!(t));
    }
    if let Some(n) = get("cursor_size").and_then(Value::as_i64)
        && n != 24
    {
        out.insert("x:cursor_size".into(), json!(n));
    }
    out
}

/// Atajos que Meca guardó en `bindings.lua`.
#[derive(Debug, Default, PartialEq)]
pub struct MecaBinds {
    /// Atajos añadidos.
    pub custom: Vec<CustomBind>,
    /// Atajos de Omarchy cambiados o desactivados: (teclas originales, nuevas o `None`).
    pub overrides: Vec<(String, Option<String>)>,
}

fn json_str(v: &Value) -> String {
    serde_json::to_string(v.as_str().unwrap_or_default()).unwrap_or_default()
}

pub fn parse_binds(text: &str) -> MecaBinds {
    let mut out = MecaBinds::default();
    let Some(meta) = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("-- MECA_KEYBINDS_META:"))
        .and_then(|j| serde_json::from_str::<Value>(j.trim()).ok())
    else {
        return out;
    };
    for c in meta
        .get("custom")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if c.get("enabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let (Some(keys), Some(cmd)) = (
            c.get("keys").and_then(Value::as_str),
            c.get("cmd").and_then(Value::as_str),
        ) else {
            continue;
        };
        out.custom.push(CustomBind {
            keys: crate::binds::normalize(keys),
            desc: c
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(cmd)
                .to_string(),
            action: json_str(&json!(cmd)),
        });
    }
    if let Some(map) = meta.get("overrides").and_then(Value::as_object) {
        for (orig, v) in map {
            let new = match v {
                Value::String(k) => Some(crate::binds::normalize(k)),
                Value::Object(o) if o.get("enabled").and_then(Value::as_bool) == Some(false) => {
                    None
                }
                Value::Object(o) => o
                    .get("keys")
                    .and_then(Value::as_str)
                    .map(crate::binds::normalize),
                _ => continue,
            };
            out.overrides.push((crate::binds::normalize(orig), new));
        }
    }
    out
}

/// `bindings.lua` sin lo que Meca gestionaba (su línea de metadatos y los
/// `o.bind` de los atajos añadidos); el resto queda igual.
pub fn clean_bindings(text: &str, binds: &MecaBinds) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("-- MECA_KEYBINDS_META:") {
            continue;
        }
        let is_meca_bind = l.strip_prefix("o.bind(").is_some_and(|r| {
            binds
                .custom
                .iter()
                .any(|c| r.starts_with(&format!("{:?}, ", c.keys)))
        });
        if !is_meca_bind {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_string() + "\n"
}

/// `hyprland.lua` sin la carga de `hyprland-gui` y su comentario.
pub fn without_gui_require(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l == "require(\"hyprland-gui\")" || l == "-- HyprMod & Meca managed settings" {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUI: &str = r#"-- Generado automáticamente por Meca HyprConfig (MECA)
-- MECA_META: {"animation_preset": "snappy", "anim_windows": "slide", "anim_workspaces_enabled": true, "anim_workspaces": "slidevert", "cursor_theme": "Yaru", "cursor_size": 32, "workspace_count": 5, "workspace_swipe": true}

hl.env("XCURSOR_THEME", "Yaru")

hl.config({
  general = {
    gaps_in = 8,
    gaps_out = 10,
    border_size = 2,
    snap = {
      enabled = true,
      window_gap = 10,
    },
  },
  decoration = {
    rounding = 6,
    rounding_power = 2.00,
    active_opacity = 0.95,
    blur = {
      enabled = true,
    },
  },
  input = {
    kb_layout = "es",
    kb_options = "compose:rwin,shift:both_capslock_cancel",
    touchpad = {
      tap_to_click = false,
    },
  },
})
"#;

    #[test]
    fn parses_nested_config() {
        let c = parse_config(GUI);
        assert_eq!(c["general:gaps_in"], json!(8));
        assert_eq!(c["general:snap:enabled"], json!(true));
        assert_eq!(c["decoration:blur:enabled"], json!(true));
        assert_eq!(c["input:touchpad:tap_to_click"], json!(false));
        assert_eq!(c["input:kb_layout"], json!("es"));
        assert_eq!(c["decoration:active_opacity"], json!(0.95));
    }

    #[test]
    fn keeps_only_differences() {
        let v = settings_from_gui(GUI);
        assert_eq!(v["general:gaps_in"], json!(8));
        assert!(!v.contains_key("general:gaps_out"), "igual a Omarchy");
        assert!(!v.contains_key("general:border_size"));
        assert_eq!(v["decoration:rounding"], json!(6));
        assert_eq!(v["decoration:blur:enabled"], json!(true));
        assert_eq!(v["input:touchpad:tap-to-click"], json!(false));
        assert_eq!(
            v["input:kb_options"],
            json!("compose:rwin,shift:both_capslock_cancel")
        );
        assert_eq!(v["x:anim_windows"], json!("slide"));
        assert_eq!(v["x:anim_workspaces"], json!("slidevert"));
        assert_eq!(v["x:anim_speed"], json!("fast"));
        assert_eq!(v["x:ws_persistent"], json!(5));
        assert_eq!(v["x:ws_swipe"], json!(true));
        assert_eq!(v["x:cursor_theme"], json!("Yaru"));
        assert_eq!(v["x:cursor_size"], json!(32));
    }

    #[test]
    fn imports_and_cleans_bindings() {
        let text = "-- Keep only your personal keybinding overrides here.\n-- MECA_KEYBINDS_META: {\"overrides\": {\"SUPER + RETURN\": \"SUPER + T\"}, \"custom\": [{\"title\": \"Antigravity\", \"keys\": \"SUPER + A\", \"cmd\": \"antigravity\", \"enabled\": true}]}\n\no.bind(\"SUPER + A\", \"Antigravity\", \"antigravity\")\no.bind(\"SUPER + B\", \"Mio\", \"otra\")\n";
        let b = parse_binds(text);
        assert_eq!(b.custom.len(), 1);
        assert_eq!(b.custom[0].keys, "SUPER + A");
        assert_eq!(b.custom[0].action, "\"antigravity\"");
        assert_eq!(
            b.overrides,
            vec![("SUPER + RETURN".to_string(), Some("SUPER + T".to_string()))]
        );
        let clean = clean_bindings(text, &b);
        assert!(
            clean.contains("SUPER + B")
                && !clean.contains("Antigravity")
                && !clean.contains("MECA")
        );
    }

    #[test]
    fn removes_gui_require() {
        let main = "require(\"hypr.autostart\")\n\n-- HyprMod & Meca managed settings\nrequire(\"hyprland-gui\")\n\n-- lizarbe\nrequire(\"hypr.x\")\n";
        let out = without_gui_require(main);
        assert!(!out.contains("hyprland-gui") && !out.contains("HyprMod"));
        assert!(out.contains("require(\"hypr.x\")"));
    }
}
