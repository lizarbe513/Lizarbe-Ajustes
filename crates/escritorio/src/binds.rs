//! Atajos de teclado: los que hay en uso (según Hyprland), la acción de cada
//! uno (leída de los archivos de Omarchy y del usuario, para poder moverlo a
//! otras teclas) y el formato de las combinaciones.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// Combinación en forma canónica: modificadores en orden fijo y en
/// mayúsculas, separados por " + " (`SUPER + SHIFT + RETURN`).
pub fn normalize(keys: &str) -> String {
    let mut mods: Vec<&str> = vec![];
    let mut rest: Vec<String> = vec![];
    for part in keys.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_uppercase().as_str() {
            "SUPER" | "WIN" | "MOD4" | "SUPER_L" | "SUPER_R" => mods.push("SUPER"),
            "CTRL" | "CONTROL" | "CTRL_L" | "CTRL_R" => mods.push("CTRL"),
            "ALT" | "MOD1" | "ALT_L" | "ALT_R" => mods.push("ALT"),
            "SHIFT" | "SHIFT_L" | "SHIFT_R" => mods.push("SHIFT"),
            _ => rest.push(key_name(part)),
        }
    }
    let order = ["SUPER", "CTRL", "ALT", "SHIFT"];
    let mut out: Vec<String> = order
        .iter()
        .filter(|m| mods.contains(m))
        .map(|m| m.to_string())
        .collect();
    out.extend(rest);
    out.join(" + ")
}

/// Nombre de tecla: letras y nombres conocidos en mayúsculas; el resto
/// (keysyms como `ntilde`, `XF86AudioMute`, `mouse:272`) tal cual.
fn key_name(k: &str) -> String {
    let up = k.to_uppercase();
    let known = [
        "RETURN",
        "SPACE",
        "TAB",
        "ESCAPE",
        "BACKSPACE",
        "DELETE",
        "INSERT",
        "HOME",
        "END",
        "PAGE_UP",
        "PAGE_DOWN",
        "UP",
        "DOWN",
        "LEFT",
        "RIGHT",
        "PRINT",
        "MINUS",
        "EQUAL",
        "SLASH",
        "COMMA",
        "PERIOD",
        "SEMICOLON",
        "APOSTROPHE",
        "GRAVE",
        "BACKSLASH",
    ];
    if k.chars().count() == 1
        || known.contains(&up.as_str())
        || (up.starts_with('F') && up[1..].parse::<u8>().is_ok())
    {
        up
    } else {
        k.to_string()
    }
}

/// Combinación a partir de la máscara de modificadores de Hyprland.
pub fn keys_from(modmask: u64, key: &str) -> String {
    let mut parts = vec![];
    for (bit, name) in [(64, "SUPER"), (4, "CTRL"), (8, "ALT"), (1, "SHIFT")] {
        if modmask & bit != 0 {
            parts.push(name);
        }
    }
    let mut s = parts.join(" + ");
    if !key.is_empty() {
        if !s.is_empty() {
            s.push_str(" + ");
        }
        s.push_str(key);
    }
    normalize(&s)
}

/// Atajo en uso.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub keys: String,
    pub desc: String,
}

/// Atajos que muestra `hyprctl binds -j` (sin submapas ni los del ratón).
pub fn parse_live(json: &str) -> Vec<Live> {
    let Ok(list) = serde_json::from_str::<Vec<Value>>(json) else {
        return vec![];
    };
    let mut out: Vec<Live> = vec![];
    for b in list {
        let s = |k: &str| {
            b.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        if !s("submap").is_empty() || b.get("mouse").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let key = s("key");
        if key.is_empty() {
            continue;
        }
        let keys = keys_from(b.get("modmask").and_then(Value::as_u64).unwrap_or(0), &key);
        if out.iter().any(|l| l.keys == keys) {
            continue;
        }
        out.push(Live {
            keys,
            desc: s("description"),
        });
    }
    out
}

pub fn live() -> Vec<Live> {
    lizarbe_core::ipc::run("hyprctl", &["binds", "-j"])
        .map(|j| parse_live(&j))
        .unwrap_or_default()
}

/// Un `o.bind(…)` leído de un archivo de configuración.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub keys: String,
    pub desc: String,
    /// Acción y opciones en Lua, si se pueden copiar a otro archivo (solo
    /// textos y tablas de valores, sin funciones ni variables locales).
    pub action: Option<String>,
    /// De qué archivo viene (`applications`, `tiling`, `user`…).
    pub origin: String,
}

/// Separa los argumentos de una llamada al nivel superior (respeta
/// comillas, llaves y paréntesis).
fn split_args(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escape = false;
    for c in s.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            '{' | '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn string_literal(s: &str) -> Option<String> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then(|| inner.to_string())
}

/// Un valor que se puede copiar: texto, número, booleano o tabla de ellos.
fn portable(expr: &str) -> bool {
    let e = expr.trim();
    if string_literal(e).is_some() || e == "true" || e == "false" || e.parse::<f64>().is_ok() {
        return true;
    }
    let Some(inner) = e.strip_prefix('{').and_then(|x| x.strip_suffix('}')) else {
        return false;
    };
    split_args(inner)
        .iter()
        .all(|field| match field.split_once('=') {
            Some((k, v)) => {
                k.trim().chars().all(|c| c.is_alphanumeric() || c == '_') && portable(v)
            }
            None => portable(field),
        })
}

/// Llamadas `o.bind("TECLAS", "Descripción", acción, opciones)` de una sola
/// línea. Las combinaciones construidas con código (bucles) no se leen.
pub fn parse_source(text: &str, origin: &str) -> Vec<Source> {
    let mut out = vec![];
    for line in text.lines() {
        let l = line.trim();
        let Some(args) = l.strip_prefix("o.bind(").and_then(|r| r.strip_suffix(')')) else {
            continue;
        };
        let args = split_args(args);
        if args.len() < 3 {
            continue;
        }
        let (Some(keys), Some(desc)) = (string_literal(&args[0]), string_literal(&args[1])) else {
            continue;
        };
        let rest = &args[2..];
        let action = rest.iter().all(|a| portable(a)).then(|| rest.join(", "));
        out.push(Source {
            keys: normalize(&keys),
            desc,
            action,
            origin: origin.to_string(),
        });
    }
    out
}

/// `o.bind` de los archivos de atajos de Omarchy y del `bindings.lua` del usuario.
pub fn sources(omarchy_path: &Path, user_bindings: &Path) -> Vec<Source> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(omarchy_path.join("default/hypr/bindings")) {
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            if f.extension().is_some_and(|e| e == "lua")
                && let (Ok(text), Some(stem)) = (std::fs::read_to_string(&f), f.file_stem())
            {
                out.extend(parse_source(&text, &stem.to_string_lossy()));
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(user_bindings) {
        out.extend(parse_source(&text, "user"));
    }
    out
}

/// Un atajo en la lista de Escritorio.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub keys: String,
    pub desc: String,
    pub origin: String,
    /// Acción que se puede llevar a otras teclas.
    pub action: Option<String>,
}

/// Une lo que está en uso con lo leído de los archivos. Lo que no aparece
/// en ningún archivo (atajos generados con código) queda como `other`.
pub fn entries(live: &[Live], sources: &[Source]) -> Vec<Entry> {
    let by_keys: HashMap<&str, &Source> = sources.iter().map(|s| (s.keys.as_str(), s)).collect();
    live.iter()
        .map(|l| {
            let src = by_keys.get(l.keys.as_str());
            Entry {
                keys: l.keys.clone(),
                desc: if l.desc.is_empty() {
                    src.map(|s| s.desc.clone()).unwrap_or_default()
                } else {
                    l.desc.clone()
                },
                origin: src
                    .map(|s| s.origin.clone())
                    .unwrap_or_else(|| "other".into()),
                action: src.and_then(|s| s.action.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_combos() {
        assert_eq!(
            normalize("shift + super + return"),
            "SUPER + SHIFT + RETURN"
        );
        assert_eq!(normalize("SUPER+ALT+p"), "SUPER + ALT + P");
        assert_eq!(normalize("CTRL + SUPER + ntilde"), "SUPER + CTRL + ntilde");
        assert_eq!(normalize("XF86AudioMute"), "XF86AudioMute");
        assert_eq!(normalize("super + f12"), "SUPER + F12");
        assert_eq!(keys_from(64 + 1, "RETURN"), "SUPER + SHIFT + RETURN");
    }

    #[test]
    fn reads_portable_bindings() {
        let text = r#"
o.bind("SUPER + RETURN", "Terminal", { omarchy = "terminal" })
o.bind( "SUPER + SHIFT + CTRL + G", "Google Messages", { webapp = "https://messages.google.com/", focus = true })
o.bind("SUPER + C", "Copy", universal_clipboard_shortcut("CTRL", "C", "CTRL + SHIFT", "C"), { repeating = true })
o.bind("SUPER + ALT + P", "Red", "nm-connection-editor --flag 123")
o.bind("SUPER + " .. workspace, "Workspace", hl.dsp.focus({ workspace = workspace }))
"#;
        let s = parse_source(text, "applications");
        assert_eq!(s.len(), 4, "la del bucle no tiene teclas literales");
        assert_eq!(s[0].keys, "SUPER + RETURN");
        assert_eq!(s[0].action.as_deref(), Some(r#"{ omarchy = "terminal" }"#));
        assert_eq!(
            s[1].action.as_deref(),
            Some(r#"{ webapp = "https://messages.google.com/", focus = true }"#)
        );
        assert_eq!(s[2].action, None, "usa una función local");
        assert_eq!(
            s[3].action.as_deref(),
            Some(r#""nm-connection-editor --flag 123""#)
        );
    }

    #[test]
    fn joins_live_and_sources() {
        let live = parse_live(
            r#"[{"modmask":64,"key":"RETURN","description":"Terminal","submap":"","mouse":false},
                {"modmask":64,"key":"mouse:272","description":"Move","submap":"","mouse":true},
                {"modmask":64,"key":"1","description":"Workspace 1","submap":"","mouse":false}]"#,
        );
        assert_eq!(live.len(), 2, "sin los del ratón");
        let src = parse_source(
            r#"o.bind("SUPER + RETURN", "Terminal", { omarchy = "terminal" })"#,
            "applications",
        );
        let e = entries(&live, &src);
        assert_eq!(e[0].origin, "applications");
        assert!(e[0].action.is_some());
        assert_eq!(e[1].origin, "other");
        assert_eq!(e[1].action, None);
    }
}
