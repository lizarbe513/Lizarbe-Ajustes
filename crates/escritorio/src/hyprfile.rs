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

use lizarbe_core::hypr::Monitor;
use serde_json::{Value, json};

use crate::capture;
use crate::catalog::{is_own, mode_value, monitor_key};

pub type Values = BTreeMap<String, Value>;

/// Lo que hace falta además de los valores para generar el archivo.
#[derive(Debug, Clone)]
pub struct RenderCtx {
    /// Pantallas conectadas (para completar sus reglas).
    pub monitors: Vec<Monitor>,
    /// Escala general de `monitors.lua` (`"auto"` o un número).
    pub global_scale: Value,
    /// Descripción y acción Lua de cada atajo, por teclas (para moverlos).
    pub bind_actions: BTreeMap<String, (String, String)>,
}

impl Default for RenderCtx {
    fn default() -> Self {
        RenderCtx {
            monitors: vec![],
            global_scale: Value::String("auto".into()),
            bind_actions: BTreeMap::new(),
        }
    }
}

/// Atajo añadido desde Escritorio.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomBind {
    pub keys: String,
    pub desc: String,
    /// Acción en Lua para `o.bind` (`{ launch = "…" }`, `"orden"`…).
    pub action: String,
}

impl CustomBind {
    pub fn to_json(&self) -> Value {
        serde_json::json!({ "keys": self.keys, "desc": self.desc, "action": self.action })
    }

    pub fn from_json(v: &Value) -> Option<CustomBind> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        Some(CustomBind {
            keys: s("keys")?,
            desc: s("desc").unwrap_or_default(),
            action: s("action")?,
        })
    }
}

pub fn custom_binds(values: &Values) -> Vec<CustomBind> {
    values
        .get("x:custom_binds")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(CustomBind::from_json).collect())
        .unwrap_or_default()
}

/// Atajos movidos, desactivados y añadidos.
fn bind_lines(values: &Values, ctx: &RenderCtx) -> Vec<String> {
    let mut unbinds = vec![];
    let mut binds = vec![];
    // Capturas: si cambió el modo, el procesado o el editor, el atajo de
    // captura lleva su propio comando (aunque no se haya movido de tecla).
    let capture = capture::command(values).map(|c| lua_value(&Value::String(c)));
    let print_key = capture::DEFAULT_KEYS;
    let print_moved = values.contains_key(&format!("x:bind:{print_key}"));
    if let (Some(action), false) = (&capture, print_moved) {
        unbinds.push(format!("hl.unbind({})", lua_value(&json!(print_key))));
        binds.push(format!(
            "o.bind({}, \"Screenshot\", {action})",
            lua_value(&json!(print_key)),
        ));
    }
    for (key, v) in values {
        let Some(orig) = key.strip_prefix("x:bind:") else {
            continue;
        };
        unbinds.push(format!(
            "hl.unbind({})",
            lua_value(&Value::String(orig.into()))
        ));
        let known = ctx.bind_actions.get(orig).cloned();
        let (desc, action) = match (orig == print_key, &capture, known) {
            (true, Some(a), k) => (
                k.map(|(d, _)| d).unwrap_or_else(|| "Screenshot".into()),
                a.clone(),
            ),
            (true, None, None) => ("Screenshot".into(), "\"omarchy-capture-screenshot\"".into()),
            (_, _, Some(k)) => k,
            _ => continue,
        };
        if let Some(new) = v.as_str() {
            binds.push(format!(
                "o.bind({}, {}, {action})",
                lua_value(&Value::String(new.into())),
                lua_value(&Value::String(desc)),
            ));
        }
    }
    for c in custom_binds(values) {
        binds.push(format!(
            "o.bind({}, {}, {})",
            lua_value(&Value::String(c.keys)),
            lua_value(&Value::String(c.desc)),
            c.action
        ));
    }
    unbinds.extend(binds);
    unbinds
}

const META: &str = "-- LIZARBE_ESCRITORIO: ";

/// Valores guardados en el archivo. `None` si no tiene la línea de
/// metadatos (no lo escribió Escritorio o está dañado).
pub fn parse(text: &str) -> Option<Values> {
    let line = text.lines().find_map(|l| l.strip_prefix(META))?;
    serde_json::from_str(line.trim()).ok()
}

/// Contenido completo del archivo para `values`.
pub fn render(values: &Values, ctx: &RenderCtx) -> String {
    let mut out = String::from(
        "-- Gestionado por Escritorio (Lizarbe): se reescribe al pulsar Aplicar.\n\
         -- No lo edites a mano; para ajustes propios usa looknfeel.lua, input.lua, etc.\n",
    );
    // Los ajustes `m:` viven en monitors.lua, no aquí.
    let stored: Values = values
        .iter()
        .filter(|(k, _)| !k.starts_with("m:") && !k.starts_with("n:") && !k.starts_with("lg:"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let meta = serde_json::to_string(&stored).unwrap_or_else(|_| "{}".into());
    let _ = writeln!(out, "{META}{meta}");

    let config: Values = values
        .iter()
        .filter(|(k, _)| !is_own(k))
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
    if values.get("x:ws_swipe").and_then(Value::as_bool) == Some(true) {
        out.push_str("\n-- Gesto de tres dedos para cambiar de escritorio\n");
        out.push_str(
            "hl.gesture({ fingers = 3, direction = \"horizontal\", action = \"workspace\" })\n",
        );
    }
    let theme = values.get("x:cursor_theme").and_then(Value::as_str);
    let size = values.get("x:cursor_size").and_then(Value::as_i64);
    if theme.is_some() || size.is_some() {
        out.push_str("\n-- Cursor\n");
        if let Some(th) = theme {
            for var in ["XCURSOR_THEME", "HYPRCURSOR_THEME"] {
                let _ = writeln!(out, "hl.env(\"{var}\", {th:?})");
            }
        }
        if let Some(sz) = size {
            for var in ["XCURSOR_SIZE", "HYPRCURSOR_SIZE"] {
                let _ = writeln!(out, "hl.env(\"{var}\", \"{sz}\")");
            }
        }
    }
    let cap_env = capture::env_vars(values);
    if !cap_env.is_empty() {
        out.push_str("\n-- Capturas de pantalla\n");
        for (k, v) in cap_env {
            let _ = writeln!(out, "hl.env({k:?}, {v:?})");
        }
    }
    let binds = bind_lines(values, ctx);
    if !binds.is_empty() {
        out.push_str("\n-- Atajos de teclado\n");
        for b in binds {
            out.push_str(&b);
            out.push('\n');
        }
    }
    let rules = monitor_rules(values, ctx);
    if !rules.is_empty() {
        out.push_str("\n-- Pantallas\n");
        for r in rules {
            out.push_str(&r);
            out.push('\n');
        }
    }
    out
}

/// Una regla `hl.monitor` completa por cada pantalla con algún ajuste. Lo
/// que no se cambió se toma de cómo está la pantalla ahora.
fn monitor_rules(values: &Values, ctx: &RenderCtx) -> Vec<String> {
    let mut names: Vec<&str> = values
        .keys()
        .filter_map(|k| monitor_key(k))
        .map(|(n, _)| n)
        .collect();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let live = ctx.monitors.iter().find(|m| m.name == name);
            let get = |part: &str| values.get(&format!("x:mon:{name}:{part}")).cloned();
            let mode = get("mode").unwrap_or_else(|| {
                Value::String(match live {
                    Some(m) => mode_value(m.width, m.height, m.refresh_rate),
                    None => "preferred".into(),
                })
            });
            let position = get("position").unwrap_or(Value::String("auto".into()));
            let scale = get("scale").unwrap_or_else(|| ctx.global_scale.clone());
            let transform = get("transform")
                .unwrap_or_else(|| Value::from(live.map(|m| m.transform).unwrap_or(0)));
            format!(
                "hl.monitor({{ output = {}, mode = {}, position = {}, scale = {}, transform = {} }})",
                lua_value(&Value::String(name.into())),
                lua_value(&mode),
                lua_value(&position),
                lua_value(&scale),
                lua_value(&transform),
            )
        })
        .collect()
}

/// Valores de las variables `local omarchy_monitor_scale` y
/// `omarchy_gdk_scale` de `monitors.lua`.
pub fn monitor_locals(text: &str) -> (Option<Value>, Option<i64>) {
    let mut scale = None;
    let mut gdk = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("local omarchy_monitor_scale = ") {
            let v = v.trim();
            scale = Some(match v.trim_matches('"') {
                s if v.starts_with('"') => Value::String(s.into()),
                s => s
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .map(Value::Number)
                    .unwrap_or(Value::String(s.into())),
            });
        } else if let Some(v) = line.strip_prefix("local omarchy_gdk_scale = ") {
            gdk = v.trim().parse().ok();
        }
    }
    (scale, gdk)
}

/// Escala de GTK (`GDK_SCALE`, solo enteros) que corresponde a la escala
/// general: la misma redondeada, como hace `omarchy-hyprland-monitor-scaling`.
/// Con `"auto"` sale de la mayor escala real de las pantallas (`live`); si no
/// hay pantallas, `None`. Si no coinciden, las apps XWayland que leen
/// `GDK_SCALE` (Steam, launchers de Java…) se ven gigantes o diminutas.
pub fn gdk_for(scale: &Value, live: &[f64]) -> Option<i64> {
    let s = scale
        .as_f64()
        .or_else(|| live.iter().copied().reduce(f64::max))?;
    Some((s.round() as i64).max(1))
}

/// `monitors.lua` con otra escala general y su escala de GTK (`gdk`; `None`
/// la deja como está). `None` si el archivo ya no tiene esas variables.
pub fn with_monitor_scale(text: &str, scale: &Value, gdk: Option<i64>) -> Option<String> {
    if !text
        .lines()
        .any(|l| l.starts_with("local omarchy_monitor_scale = "))
    {
        return None;
    }
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("local omarchy_monitor_scale = ") {
            let _ = writeln!(out, "local omarchy_monitor_scale = {}", lua_value(scale));
        } else if line.starts_with("local omarchy_gdk_scale = ")
            && let Some(g) = gdk
        {
            let _ = writeln!(out, "local omarchy_gdk_scale = {g}");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Some(out)
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

/// Número sin ceros sobrantes y con hasta 6 decimales (las escalas de
/// pantalla necesitan precisión): 0.5 → "0.5", 3.0 → "3".
pub fn num(n: f64) -> String {
    let s = format!("{n:.6}");
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
    fn screenshot_command_goes_into_the_capture_bind() {
        let ctx = RenderCtx::default();
        // sin cambios, nada
        assert!(!render(&Values::new(), &ctx).contains("omarchy-capture-screenshot"));
        // solo el modo: se suelta PRINT y se vuelve a poner con el comando
        let t = render(&vals(&[("x:cap_mode", json!("region"))]), &ctx);
        assert!(t.contains("hl.unbind(\"PRINT\")"), "{t}");
        assert!(
            t.contains(
                "o.bind(\"PRINT\", \"Screenshot\", \"omarchy-capture-screenshot region slurp\")"
            ),
            "{t}"
        );
        // tecla movida + solo copiar: una sola suelta y la tecla nueva
        let t = render(
            &vals(&[
                ("x:bind:PRINT", json!("SUPER + S")),
                ("x:cap_proc", json!("copy")),
            ]),
            &ctx,
        );
        assert_eq!(t.matches("hl.unbind(\"PRINT\")").count(), 1, "{t}");
        assert!(
            t.contains(
                "o.bind(\"SUPER + S\", \"Screenshot\", \"omarchy-capture-screenshot smart copy\")"
            ),
            "{t}"
        );
        // tecla desactivada: no se vuelve a poner
        let t = render(
            &vals(&[("x:bind:PRINT", Value::Null), ("x:cap_proc", json!("copy"))]),
            &ctx,
        );
        assert!(!t.contains("o.bind"), "{t}");
    }

    #[test]
    fn screenshot_folder_is_exported_to_hyprland() {
        let t = render(
            &vals(&[("x:cap_dir", json!("/tmp/capturas"))]),
            &RenderCtx::default(),
        );
        assert!(
            t.contains("hl.env(\"OMARCHY_SCREENSHOT_DIR\", \"/tmp/capturas\")"),
            "{t}"
        );
    }

    #[test]
    fn the_stored_screenshot_values_are_not_hyprland_options() {
        let t = render(
            &vals(&[("x:cap_dir", json!("/x")), ("x:cap_mode", json!("region"))]),
            &RenderCtx::default(),
        );
        assert!(!t.contains("hl.config"), "{t}");
        assert!(t.contains("LIZARBE_ESCRITORIO"));
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
        let text = render(&v, &RenderCtx::default());
        assert_eq!(parse(&text), Some(v));
        assert!(text.contains("  general = {\n    gaps_in = 8,\n    layout = \"master\",\n  },"));
        assert!(text.contains("blur = {\n      enabled = true,"));
        assert!(text.contains("active_opacity = 0.95,"));
        assert!(text.contains("tap_to_click = false,"), "guion → guion bajo");
    }

    #[test]
    fn empty_has_no_config_block() {
        let text = render(&Values::new(), &RenderCtx::default());
        assert_eq!(parse(&text), Some(Values::new()));
        assert!(!text.contains("hl.config"));
        assert_eq!(parse("-- otro archivo\n"), None);
    }

    #[test]
    fn animations_only_when_changed() {
        let v = vals(&[("x:anim_speed", json!("fast"))]);
        let text = render(&v, &RenderCtx::default());
        assert!(text.contains("leaf = \"windows\", enabled = true, speed = 2.274"));
        assert!(!text.contains("leaf = \"workspaces\""));
        let v = vals(&[("x:anim_windows", json!("slide"))]);
        let text = render(&v, &RenderCtx::default());
        assert!(text.contains("leaf = \"windowsIn\", enabled = true, speed = 4.1, bezier = \"easeOutQuint\", style = \"slide\""));
        assert!(
            !text.contains("leaf = \"fade\""),
            "solo las hojas afectadas"
        );
        let v = vals(&[("x:anim_workspaces", json!("slidevert"))]);
        assert!(render(&v, &RenderCtx::default()).contains("leaf = \"workspaces\", enabled = true, speed = 3, bezier = \"easeOutQuint\", style = \"slidevert\""));
    }

    #[test]
    fn persistent_workspaces() {
        let text = render(
            &vals(&[("x:ws_persistent", json!(3))]),
            &RenderCtx::default(),
        );
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
    fn monitor_rules_fill_from_live() {
        let mut m = Monitor::named("HDMI-A-1");
        m.width = 1920;
        m.height = 1080;
        m.refresh_rate = 60.0;
        let ctx = RenderCtx {
            monitors: vec![m],
            global_scale: json!(1.25),
            ..Default::default()
        };
        let text = render(
            &vals(&[("x:mon:HDMI-A-1:position", json!("auto-left"))]),
            &ctx,
        );
        assert!(text.contains(
            "hl.monitor({ output = \"HDMI-A-1\", mode = \"1920x1080@60.00\", position = \"auto-left\", scale = 1.25, transform = 0 })"
        ));
        // Desconectada: modo preferido.
        let text = render(&vals(&[("x:mon:DP-1:scale", json!(2))]), &ctx);
        assert!(
            text.contains(
                "output = \"DP-1\", mode = \"preferred\", position = \"auto\", scale = 2"
            )
        );
    }

    #[test]
    fn edits_monitor_locals() {
        let text = "local omarchy_gdk_scale = 2\nlocal omarchy_monitor_scale = \"auto\"\nhl.env(\"GDK_SCALE\", tostring(omarchy_gdk_scale))\n";
        assert_eq!(monitor_locals(text), (Some(json!("auto")), Some(2)));
        let out = with_monitor_scale(text, &json!(1.25), gdk_for(&json!(1.25), &[])).unwrap();
        assert!(
            out.starts_with("local omarchy_gdk_scale = 1\nlocal omarchy_monitor_scale = 1.25\n")
        );
        assert_eq!(monitor_locals(&out), (Some(json!(1.25)), Some(1)));
        let out = with_monitor_scale(text, &json!("auto"), None).unwrap();
        assert!(
            out.contains("omarchy_gdk_scale = 2"),
            "sin pantallas, con auto se mantiene la de GTK"
        );
        assert_eq!(
            with_monitor_scale("hl.monitor({})\n", &json!(1), Some(1)),
            None
        );
    }

    #[test]
    fn gdk_follows_real_scale() {
        assert_eq!(gdk_for(&json!(1.6), &[1.0]), Some(2));
        assert_eq!(gdk_for(&json!(1.25), &[]), Some(1));
        // `auto` en un 1080p: Hyprland usa 1, así que GTK también.
        assert_eq!(gdk_for(&json!("auto"), &[1.0]), Some(1));
        assert_eq!(gdk_for(&json!("auto"), &[1.0, 2.0]), Some(2));
        assert_eq!(gdk_for(&json!("auto"), &[]), None);
    }

    #[test]
    fn moves_disables_and_adds_binds() {
        let mut ctx = RenderCtx::default();
        ctx.bind_actions.insert(
            "SUPER + RETURN".into(),
            ("Terminal".into(), "{ omarchy = \"terminal\" }".into()),
        );
        let custom = CustomBind {
            keys: "SUPER + ALT + W".into(),
            desc: "WhatsApp".into(),
            action: "{ webapp = \"https://web.whatsapp.com/\" }".into(),
        };
        let text = render(
            &vals(&[
                ("x:bind:SUPER + RETURN", json!("SUPER + T")),
                ("x:bind:SUPER + SHIFT + Y", Value::Null),
                ("x:custom_binds", json!([custom.to_json()])),
            ]),
            &ctx,
        );
        let i_unbind = text.find("hl.unbind(\"SUPER + RETURN\")").unwrap();
        let i_bind = text
            .find("o.bind(\"SUPER + T\", \"Terminal\", { omarchy = \"terminal\" })")
            .unwrap();
        assert!(i_unbind < i_bind, "primero se quitan, luego se asignan");
        assert!(text.contains("hl.unbind(\"SUPER + SHIFT + Y\")"));
        assert!(text.contains(
            "o.bind(\"SUPER + ALT + W\", \"WhatsApp\", { webapp = \"https://web.whatsapp.com/\" })"
        ));
        assert!(!text.contains("hl.config"));
    }

    #[test]
    fn cursor_and_gesture_lines() {
        let text = render(
            &vals(&[
                ("x:cursor_theme", json!("Yaru")),
                ("x:cursor_size", json!(32)),
                ("x:ws_swipe", json!(true)),
                ("m:scale", json!(2)),
            ]),
            &RenderCtx::default(),
        );
        assert!(text.contains("hl.env(\"XCURSOR_THEME\", \"Yaru\")"));
        assert!(text.contains("hl.env(\"HYPRCURSOR_SIZE\", \"32\")"));
        assert!(text.contains("hl.gesture({ fingers = 3"));
        assert!(
            !text.contains("m:scale"),
            "m: no se guarda en escritorio.lua"
        );
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(3.0), "3");
        assert_eq!(num(3.79 * 0.6), "2.274");
        assert_eq!(num(160.0 / 120.0), "1.333333");
        assert_eq!(lua_value(&json!([4, 3])), "{ 4, 3 }");
        assert_eq!(lua_value(&json!("a\"b")), "\"a\\\"b\"");
    }
}
