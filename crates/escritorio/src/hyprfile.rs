//! `~/.config/hypr/escritorio.lua`: el archivo que Escritorio gestiona.
//!
//! Solo guarda los ajustes que el usuario cambió respecto a Omarchy. La
//! fuente de verdad es la línea de metadatos JSON; el Lua se genera a partir
//! de ella, así que no hace falta interpretar Lua para leerlo.
//!
//! Las claves son nombres de opción de Hyprland (`general:gaps_in`) o
//! ajustes propios con prefijo `x:` (animaciones, escritorios fijos) que se
//! traducen a llamadas `hl.animation` / `hl.workspace_rule`.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::Value;

pub type Values = BTreeMap<String, Value>;

const META: &str = "-- LIZARBE_ESCRITORIO: ";

/// Valores guardados en el archivo. `None` si no tiene la línea de
/// metadatos (no lo escribió Escritorio o está dañado).
pub fn parse(text: &str) -> Option<Values> {
    let line = text.lines().find_map(|l| l.strip_prefix(META))?;
    serde_json::from_str(line.trim()).ok()
}

/// Contenido completo del archivo para `values`.
pub fn render(values: &Values) -> String {
    let mut out = String::from(
        "-- Gestionado por Escritorio (Lizarbe): se reescribe al pulsar Aplicar.\n\
         -- No lo edites a mano; para ajustes propios usa looknfeel.lua, input.lua, etc.\n",
    );
    let meta = serde_json::to_string(values).unwrap_or_else(|_| "{}".into());
    let _ = writeln!(out, "{META}{meta}");

    let config: Values = values
        .iter()
        .filter(|(k, _)| !k.starts_with("x:"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if !config.is_empty() {
        out.push_str("\nhl.config(");
        out.push_str(&lua_table(&tree(&config), 0));
        out.push_str(")\n");
    }
    let anims = animations(values);
    if !anims.is_empty() {
        out.push_str("\n-- Animaciones\n");
        for a in anims {
            out.push_str(&a);
            out.push('\n');
        }
    }
    if let Some(n) = values.get("x:ws_persistent").and_then(Value::as_u64)
        && n > 0
    {
        out.push_str("\n-- Escritorios que existen aunque estén vacíos\n");
        for i in 1..=n.min(10) {
            let _ = writeln!(
                out,
                "hl.workspace_rule({{ workspace = \"{i}\", persistent = true }})"
            );
        }
    }
    out
}

/// Árbol anidado a partir de claves `a:b:c` o `a:b.c` (los guiones pasan a
/// guion bajo, como espera la API Lua de Hyprland).
enum Node {
    Leaf(Value),
    Table(BTreeMap<String, Node>),
}

fn tree(values: &Values) -> BTreeMap<String, Node> {
    let mut root: BTreeMap<String, Node> = BTreeMap::new();
    for (key, v) in values {
        let parts: Vec<String> = key.split([':', '.']).map(|p| p.replace('-', "_")).collect();
        let mut cur = &mut root;
        for (i, p) in parts.iter().enumerate() {
            if i + 1 == parts.len() {
                cur.insert(p.clone(), Node::Leaf(v.clone()));
            } else {
                let entry = cur
                    .entry(p.clone())
                    .or_insert_with(|| Node::Table(BTreeMap::new()));
                if let Node::Leaf(_) = entry {
                    *entry = Node::Table(BTreeMap::new());
                }
                let Node::Table(next) = entry else {
                    unreachable!()
                };
                cur = next;
            }
        }
    }
    root
}

fn lua_table(map: &BTreeMap<String, Node>, depth: usize) -> String {
    let pad = "  ".repeat(depth + 1);
    let mut s = String::from("{\n");
    for (k, node) in map {
        let val = match node {
            Node::Leaf(v) => lua_value(v),
            Node::Table(t) => lua_table(t, depth + 1),
        };
        let _ = writeln!(s, "{pad}{k} = {val},");
    }
    s.push_str(&"  ".repeat(depth));
    s.push('}');
    s
}

/// Número sin ceros sobrantes: 0.5 → "0.5", 3.0 → "3", 2.274 → "2.27".
pub fn num(n: f64) -> String {
    let s = format!("{:.2}", (n * 100.0).round() / 100.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

pub fn lua_value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n
            .as_i64()
            .map(|i| i.to_string())
            .unwrap_or_else(|| num(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => format!("{s:?}"),
        Value::Array(a) => format!(
            "{{ {} }}",
            a.iter().map(lua_value).collect::<Vec<_>>().join(", ")
        ),
        Value::Null => "nil".into(),
        Value::Object(_) => "nil".into(),
    }
}

/// Animaciones de Omarchy: (hoja, velocidad, curva, estilo). La velocidad
/// es la duración en décimas de segundo: más alta = más lenta.
const ANIMS: [(&str, f64, &str, Option<&str>); 7] = [
    ("windows", 3.79, "easeOutQuint", None),
    ("windowsIn", 4.1, "easeOutQuint", Some("popin 87%")),
    ("windowsOut", 1.49, "linear", Some("popin 87%")),
    ("fade", 3.03, "quick", None),
    ("layersIn", 4.0, "easeOutQuint", Some("fade")),
    ("layersOut", 1.5, "linear", Some("fade")),
    ("specialWorkspace", 3.0, "easeOutQuint", Some("slidevert")),
];

/// Factor de duración según `x:anim_speed`.
pub fn speed_factor(values: &Values) -> f64 {
    match values.get("x:anim_speed").and_then(Value::as_str) {
        Some("slow") => 1.5,
        Some("fast") => 0.6,
        _ => 1.0,
    }
}

fn animations(values: &Values) -> Vec<String> {
    let factor = speed_factor(values);
    let windows = values.get("x:anim_windows").and_then(Value::as_str);
    let workspaces = values.get("x:anim_workspaces").and_then(Value::as_str);
    let mut out = vec![];
    let line = |leaf: &str, speed: f64, bezier: &str, style: Option<&str>| {
        let style = style
            .map(|s| format!(", style = {s:?}"))
            .unwrap_or_default();
        format!(
            "hl.animation({{ leaf = \"{leaf}\", enabled = true, speed = {}, bezier = \"{bezier}\"{style} }})",
            num(speed * factor)
        )
    };
    for (leaf, speed, bezier, style) in ANIMS {
        let custom_style = match leaf {
            "windowsIn" | "windowsOut" => windows,
            _ => None,
        };
        if factor != 1.0 || custom_style.is_some() {
            out.push(line(leaf, speed, bezier, custom_style.or(style)));
        }
    }
    match workspaces {
        Some("off") => out.push("hl.animation({ leaf = \"workspaces\", enabled = false })".into()),
        Some(style) => out.push(line("workspaces", 3.0, "easeOutQuint", Some(style))),
        None => {}
    }
    out
}

/// Inserta `require("hypr.escritorio")` en `hyprland.lua` antes de los
/// toggles de Omarchy (para que sigan pudiendo pisar estos valores) o al
/// final. `None` si ya está.
pub fn with_require(main: &str) -> Option<String> {
    const REQ: &str = "require(\"hypr.escritorio\")";
    if main.lines().any(|l| l.trim() == REQ) {
        return None;
    }
    let mut out = String::new();
    let mut done = false;
    for line in main.lines() {
        if !done && line.trim() == "require(\"default.hypr.toggles\")" {
            out.push_str(REQ);
            out.push('\n');
            done = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !done {
        if !out.ends_with("\n\n") && !out.is_empty() {
            out.push('\n');
        }
        out.push_str("-- Escritorio (Lizarbe)\n");
        out.push_str(REQ);
        out.push('\n');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vals(pairs: &[(&str, Value)]) -> Values {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn roundtrip_and_nested_lua() {
        let v = vals(&[
            ("general:gaps_in", json!(8)),
            ("decoration:blur:enabled", json!(true)),
            ("decoration:active_opacity", json!(0.95)),
            ("input:touchpad:tap-to-click", json!(false)),
            ("general:layout", json!("master")),
        ]);
        let text = render(&v);
        assert_eq!(parse(&text), Some(v));
        assert!(text.contains("  general = {\n    gaps_in = 8,\n    layout = \"master\",\n  },"));
        assert!(text.contains("blur = {\n      enabled = true,"));
        assert!(text.contains("active_opacity = 0.95,"));
        assert!(text.contains("tap_to_click = false,"), "guion → guion bajo");
    }

    #[test]
    fn empty_has_no_config_block() {
        let text = render(&Values::new());
        assert_eq!(parse(&text), Some(Values::new()));
        assert!(!text.contains("hl.config"));
        assert_eq!(parse("-- otro archivo\n"), None);
    }

    #[test]
    fn animations_only_when_changed() {
        let v = vals(&[("x:anim_speed", json!("fast"))]);
        let text = render(&v);
        assert!(text.contains("leaf = \"windows\", enabled = true, speed = 2.27"));
        assert!(!text.contains("leaf = \"workspaces\""));
        let v = vals(&[("x:anim_windows", json!("slide"))]);
        let text = render(&v);
        assert!(text.contains("leaf = \"windowsIn\", enabled = true, speed = 4.1, bezier = \"easeOutQuint\", style = \"slide\""));
        assert!(
            !text.contains("leaf = \"fade\""),
            "solo las hojas afectadas"
        );
        let v = vals(&[("x:anim_workspaces", json!("slidevert"))]);
        assert!(render(&v).contains("leaf = \"workspaces\", enabled = true, speed = 3, bezier = \"easeOutQuint\", style = \"slidevert\""));
    }

    #[test]
    fn persistent_workspaces() {
        let text = render(&vals(&[("x:ws_persistent", json!(3))]));
        assert_eq!(text.matches("persistent = true").count(), 3);
        assert!(
            !text.contains("hl.config"),
            "los ajustes x: no van en hl.config"
        );
    }

    #[test]
    fn inserts_require_before_toggles() {
        let main = "require(\"hypr.looknfeel\")\nrequire(\"default.hypr.toggles\")\nrequire(\"hyprland-gui\")\n";
        let out = with_require(main).unwrap();
        assert_eq!(
            out,
            "require(\"hypr.looknfeel\")\nrequire(\"hypr.escritorio\")\nrequire(\"default.hypr.toggles\")\nrequire(\"hyprland-gui\")\n"
        );
        assert_eq!(with_require(&out), None, "idempotente");
        let out = with_require("require(\"hypr.looknfeel\")\n").unwrap();
        assert!(out.ends_with("-- Escritorio (Lizarbe)\nrequire(\"hypr.escritorio\")\n"));
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(3.0), "3");
        assert_eq!(num(3.79 * 0.6), "2.27");
        assert_eq!(lua_value(&json!([4, 3])), "{ 4, 3 }");
        assert_eq!(lua_value(&json!("a\"b")), "\"a\\\"b\"");
    }
}
