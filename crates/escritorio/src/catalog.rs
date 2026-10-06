//! Ajustes que ofrece Escritorio, agrupados por sección.
//!
//! Cada ajuste es una opción de Hyprland (`general:gaps_in`) o un ajuste
//! propio `x:` (ver `hyprfile`). El valor por defecto es el de Omarchy (o el
//! de Hyprland si Omarchy no lo cambia): es al que se vuelve al restaurar.
//! Los textos salen de `i18n`: `o.<clave>` (nombre), `o.<clave>.d`
//! (descripción) y `o.<clave>=<valor>` (cada opción de una lista).

use lizarbe_core::hypr::Monitor;
use lizarbe_core::schema::{FieldDef, Kind, Opt, float, int};
use serde_json::{Value, json};

use crate::i18n::{t, tf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Windows,
    Monitors,
    Keyboard,
    Mouse,
    Cursor,
    Changes,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::Appearance,
        Section::Windows,
        Section::Monitors,
        Section::Keyboard,
        Section::Mouse,
        Section::Cursor,
        Section::Changes,
    ];

    pub fn index(self) -> usize {
        Section::ALL.iter().position(|s| *s == self).unwrap()
    }

    /// Identificador para `--section` (en español o en inglés).
    pub fn from_id(id: &str) -> Option<Section> {
        Some(match id.trim().to_lowercase().as_str() {
            "apariencia" | "appearance" => Section::Appearance,
            "ventanas" | "windows" => Section::Windows,
            "pantallas" | "monitors" | "monitores" => Section::Monitors,
            "teclado" | "keyboard" => Section::Keyboard,
            "mouse" | "raton" | "ratón" | "touchpad" => Section::Mouse,
            "cursor" => Section::Cursor,
            "cambios" | "changes" => Section::Changes,
            _ => return None,
        })
    }

    fn id(self) -> &'static str {
        match self {
            Section::Appearance => "appearance",
            Section::Windows => "windows",
            Section::Monitors => "monitors",
            Section::Keyboard => "keyboard",
            Section::Mouse => "mouse",
            Section::Cursor => "cursor",
            Section::Changes => "changes",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Section::Appearance => "󰏘",
            Section::Windows => "󰕮",
            Section::Monitors => "󰍹",
            Section::Keyboard => "󰌌",
            Section::Mouse => "󰍽",
            Section::Cursor => "󰇀",
            Section::Changes => "󰄬",
        }
    }

    pub fn title(self) -> String {
        t(&format!("sec.{}", self.id()))
    }

    pub fn description(self) -> String {
        t(&format!("sec.{}.d", self.id()))
    }
}

/// Categorías de la barra lateral: (clave de texto, secciones). El orden
/// coincide con `Section::ALL`.
pub const CATEGORIES: [(&str, &[Section]); 3] = [
    ("cat.desktop", &[Section::Appearance, Section::Windows]),
    (
        "cat.devices",
        &[
            Section::Monitors,
            Section::Keyboard,
            Section::Mouse,
            Section::Cursor,
        ],
    ),
    ("cat.review", &[Section::Changes]),
];

/// Lo que depende del equipo: pantallas conectadas y temas de cursor.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub monitors: Vec<Monitor>,
    pub cursor_themes: Vec<String>,
}

/// Grupo de ajustes con su encabezado.
pub struct Group {
    pub id: String,
    pub title: String,
    pub fields: Vec<FieldDef>,
}

impl Group {
    fn new(id: &str, fields: Vec<FieldDef>) -> Self {
        Group {
            id: id.to_string(),
            title: t(&format!("g.{id}")),
            fields,
        }
    }
}

fn field(key: &str, kind: Kind, default: Value) -> FieldDef {
    FieldDef::new(key, t(&format!("o.{key}")), kind)
        .desc(t(&format!("o.{key}.d")))
        .default(default)
}

fn toggle(key: &str, default: bool) -> FieldDef {
    field(key, Kind::Bool, json!(default))
}

fn steps(key: &str, min: i64, max: i64, step: i64, default: i64) -> FieldDef {
    field(key, int(Some(min), Some(max), step), json!(default))
}

fn slider(key: &str, min: f64, max: f64, step: f64, default: f64) -> FieldDef {
    field(key, float(Some(min), Some(max), step), json!(default))
}

/// Lista de opciones; cada una con su texto `o.<clave>=<valor>`.
fn choice(key: &str, values: &[Value], default: Value) -> FieldDef {
    let opts = values
        .iter()
        .map(|v| {
            let id = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Opt::new(v.clone(), t(&format!("o.{key}={id}")))
        })
        .collect();
    field(key, Kind::Enum(opts), default)
}

/// Texto con sugerencias y la opción "Personalizado…".
fn text_presets(key: &str, values: &[&str], default: &str) -> FieldDef {
    let presets = values
        .iter()
        .map(|v| Opt::new(json!(v), t(&format!("o.{key}={v}"))))
        .collect();
    field(key, Kind::Text, json!(default)).presets(presets)
}

pub fn groups(section: Section, ctx: &Ctx) -> Vec<Group> {
    match section {
        Section::Appearance => appearance(),
        Section::Windows => windows(),
        Section::Monitors => monitors(ctx),
        Section::Keyboard => keyboard(),
        Section::Mouse => mouse(),
        Section::Cursor => cursor(ctx),
        Section::Changes => vec![],
    }
}

fn appearance() -> Vec<Group> {
    vec![
        Group::new(
            "spacing",
            vec![
                steps("general:gaps_in", 0, 30, 1, 5),
                steps("general:gaps_out", 0, 50, 1, 10),
                steps("general:border_size", 0, 10, 1, 2),
                steps("decoration:rounding", 0, 30, 1, 0),
                slider("decoration:rounding_power", 1.0, 5.0, 0.25, 2.0).advanced(),
            ],
        ),
        Group::new(
            "opacity",
            vec![
                slider("decoration:active_opacity", 0.1, 1.0, 0.05, 1.0),
                slider("decoration:inactive_opacity", 0.1, 1.0, 0.05, 1.0),
                slider("decoration:fullscreen_opacity", 0.1, 1.0, 0.05, 1.0).advanced(),
                toggle("decoration:dim_inactive", false),
                slider("decoration:dim_strength", 0.0, 1.0, 0.05, 0.5),
                slider("decoration:dim_special", 0.0, 1.0, 0.05, 0.2).advanced(),
            ],
        ),
        Group::new(
            "blur",
            vec![
                toggle("decoration:blur:enabled", false),
                steps("decoration:blur:size", 1, 20, 1, 8),
                steps("decoration:blur:passes", 1, 6, 1, 1),
                slider("decoration:blur:vibrancy", 0.0, 1.0, 0.05, 0.17).advanced(),
                toggle("decoration:blur:xray", false).advanced(),
                toggle("decoration:blur:ignore_opacity", true).advanced(),
                toggle("decoration:blur:new_optimizations", true).advanced(),
            ],
        ),
        Group::new(
            "shadow",
            vec![
                toggle("decoration:shadow:enabled", false),
                steps("decoration:shadow:range", 1, 40, 1, 4),
                steps("decoration:shadow:render_power", 1, 4, 1, 3).advanced(),
                toggle("decoration:shadow:sharp", false).advanced(),
            ],
        ),
        Group::new(
            "animations",
            vec![
                toggle("animations:enabled", true),
                choice(
                    "x:anim_speed",
                    &[json!("slow"), json!("normal"), json!("fast")],
                    json!("normal"),
                ),
                choice(
                    "x:anim_windows",
                    &[
                        json!("popin 87%"),
                        json!("popin 60%"),
                        json!("slide"),
                        json!("gnomed"),
                    ],
                    json!("popin 87%"),
                ),
                choice(
                    "x:anim_workspaces",
                    &[
                        json!("off"),
                        json!("slide"),
                        json!("slidevert"),
                        json!("fade"),
                        json!("slidefade 20%"),
                    ],
                    json!("off"),
                ),
                toggle("animations:workspace_wraparound", false).advanced(),
            ],
        ),
    ]
}

fn windows() -> Vec<Group> {
    vec![
        Group::new(
            "layout",
            vec![
                choice(
                    "general:layout",
                    &[json!("dwindle"), json!("master"), json!("scrolling")],
                    json!("dwindle"),
                ),
                choice(
                    "dwindle:force_split",
                    &[json!(0), json!(1), json!(2)],
                    json!(2),
                ),
                toggle("dwindle:preserve_split", true),
                toggle("dwindle:smart_split", false).advanced(),
                toggle("dwindle:smart_resizing", true).advanced(),
                slider("dwindle:default_split_ratio", 0.5, 1.5, 0.05, 1.0).advanced(),
                slider("master:mfact", 0.2, 0.8, 0.05, 0.55),
                choice(
                    "master:orientation",
                    &[
                        json!("left"),
                        json!("right"),
                        json!("top"),
                        json!("bottom"),
                        json!("center"),
                    ],
                    json!("left"),
                ),
                choice(
                    "master:new_status",
                    &[json!("master"), json!("slave"), json!("inherit")],
                    json!("master"),
                )
                .advanced(),
                slider("scrolling:column_width", 0.25, 1.0, 0.05, 0.49),
            ],
        ),
        Group::new(
            "moving",
            vec![
                toggle("general:resize_on_border", false),
                steps("general:extend_border_grab_area", 0, 30, 1, 15).advanced(),
                toggle("general:hover_icon_on_border", true).advanced(),
                toggle("general:snap:enabled", false),
                steps("general:snap:window_gap", 0, 40, 2, 10).advanced(),
                steps("general:snap:monitor_gap", 0, 40, 2, 10).advanced(),
                toggle("general:snap:border_overlap", false).advanced(),
                toggle("misc:animate_manual_resizes", false).advanced(),
                toggle("misc:animate_mouse_windowdragging", false).advanced(),
            ],
        ),
        Group::new(
            "workspaces",
            vec![
                steps("x:ws_persistent", 0, 10, 1, 0),
                toggle("binds:workspace_back_and_forth", false),
                toggle("binds:allow_workspace_cycles", false).advanced(),
                toggle("binds:hide_special_on_workspace_change", true).advanced(),
            ],
        ),
        Group::new(
            "focus",
            vec![
                toggle("misc:focus_on_activate", true),
                choice(
                    "misc:on_focus_under_fullscreen",
                    &[json!(0), json!(1), json!(2)],
                    json!(1),
                )
                .advanced(),
                toggle("general:no_focus_fallback", false).advanced(),
                toggle("general:allow_tearing", false).advanced(),
            ],
        ),
        Group::new(
            "groups",
            vec![
                toggle("group:groupbar:enabled", true),
                steps("group:groupbar:font_size", 8, 20, 1, 12).advanced(),
                steps("group:groupbar:height", 14, 36, 2, 22).advanced(),
                toggle("group:groupbar:gradients", true).advanced(),
            ],
        ),
    ]
}

/// Opción de Hyprland que el catálogo no muestra tal cual: Teclado la edita
/// por partes (tecla Compose y cambio de idioma).
pub fn kb_options() -> FieldDef {
    field(
        "input:kb_options",
        Kind::Text,
        json!("compose:caps,shift:both_capslock_cancel"),
    )
}

/// Todas las opciones de Hyprland del catálogo (sin las `x:`, `m:` ni
/// `kbopt:`), para leer sus valores en uso.
pub fn hypr_keys() -> Vec<String> {
    let ctx = Ctx::default();
    let mut keys: Vec<String> = Section::ALL
        .iter()
        .flat_map(|s| groups(*s, &ctx))
        .flat_map(|g| g.fields)
        .map(|f| f.key)
        .filter(|k| k.contains(':') && !is_own(k))
        .collect();
    keys.push("input:kb_options".into());
    keys
}

/// Ajustes propios de Escritorio, que no son opciones de Hyprland.
pub fn is_own(key: &str) -> bool {
    key.starts_with("x:") || key.starts_with("m:") || key.starts_with("kbopt:")
}

/// Definición de un ajuste por su clave.
pub fn find(key: &str, ctx: &Ctx) -> Option<FieldDef> {
    if key == "input:kb_options" {
        return Some(kb_options());
    }
    if let Some(def) = Section::ALL
        .iter()
        .flat_map(|s| groups(*s, ctx))
        .flat_map(|g| g.fields)
        .find(|f| f.key == key)
    {
        return Some(def);
    }
    // Pantalla desconectada: su ajuste guardado sigue teniendo nombre.
    let (name, part) = monitor_key(key)?;
    let mut def = monitor_field(&Monitor::named(name), part)?;
    def.label = tf("mon.label", &[("name", name), ("field", &def.label)]);
    Some(def)
}

/// Convierte el valor que da Hyprland al tipo del campo (p. ej. un tiempo
/// llega como 0.0 y el campo usa enteros).
pub fn coerce(kind: &Kind, v: Value) -> Value {
    let int_like = |v: &Value| {
        v.as_f64()
            .filter(|f| f.fract() == 0.0)
            .map(|f| json!(f as i64))
    };
    match kind {
        Kind::Int { .. } => int_like(&v).unwrap_or(v),
        Kind::Float { .. } => v.as_f64().map(|f| json!(f)).unwrap_or(v),
        Kind::Bool => match &v {
            Value::Number(n) => json!(n.as_f64().unwrap_or(0.0) != 0.0),
            _ => v,
        },
        Kind::Enum(opts) if opts.iter().any(|o| o.value.is_i64()) => int_like(&v).unwrap_or(v),
        _ => v,
    }
}

// ---------------------------------------------------------------- pantallas

/// Separa `x:mon:<pantalla>:<campo>`.
pub fn monitor_key(key: &str) -> Option<(&str, &str)> {
    let rest = key.strip_prefix("x:mon:")?;
    let (name, part) = rest.rsplit_once(':')?;
    Some((name, part))
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Escala válida más cercana hacia arriba: Hyprland solo acepta escalas que
/// dividen la resolución en píxeles lógicos enteros (en pasos de 1/120), igual
/// que `omarchy-hyprland-monitor-scaling`.
pub fn clean_scale(scale: f64, width: u32, height: u32) -> f64 {
    let g = gcd(width as u64 * 120, height as u64 * 120).max(1);
    let mut k = ((scale * 120.0).round() as u64).clamp(1, g);
    while !g.is_multiple_of(k) {
        k += 1;
    }
    k as f64 / 120.0
}

/// Escalas que se ofrecen para una resolución (las de Omarchy, ajustadas).
pub fn scale_options(width: u32, height: u32) -> Vec<f64> {
    let mut out: Vec<f64> = vec![];
    for s in [1.0, 1.25, 1.6, 2.0, 3.0] {
        let c = clean_scale(s, width, height);
        if !out.iter().any(|x| (x - c).abs() < 1e-6) {
            out.push(c);
        }
    }
    out
}

fn percent(scale: f64) -> String {
    format!("{} %", (scale * 100.0).round())
}

fn scale_opts(scales: &[f64]) -> Vec<Opt> {
    scales
        .iter()
        .map(|s| Opt::new(json!(s), percent(*s)))
        .collect()
}

/// Frecuencia legible: 60 → "60", 59.94 → "59.94".
fn hz_label(hz: f64) -> String {
    let s = format!("{hz:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Modo como lo escribe Hyprland en una regla: `1920x1080@60.00`.
pub fn mode_value(width: u32, height: u32, hz: f64) -> String {
    format!("{width}x{height}@{hz:.2}")
}

/// `1920x1080@60.00Hz` → (ancho, alto, Hz).
pub fn parse_mode(mode: &str) -> Option<(u32, u32, f64)> {
    let mode = mode.trim_end_matches("Hz");
    let (size, hz) = mode.split_once('@')?;
    let (w, h) = size.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, hz.parse().ok()?))
}

fn monitor_field(m: &Monitor, part: &str) -> Option<FieldDef> {
    let key = format!("x:mon:{}:{part}", m.name);
    let with = |kind: Kind, default: Value| {
        FieldDef::new(&key, t(&format!("mon.{part}")), kind)
            .desc(t(&format!("mon.{part}.d")))
            .default(default)
    };
    Some(match part {
        "mode" => {
            let mut opts: Vec<Opt> = vec![];
            for mode in &m.available_modes {
                if let Some((w, h, hz)) = parse_mode(mode) {
                    let v = mode_value(w, h, hz);
                    if !opts.iter().any(|o| o.value == json!(v)) {
                        opts.push(Opt::new(
                            json!(v),
                            tf(
                                "mon.mode.opt",
                                &[
                                    ("w", &w.to_string()),
                                    ("h", &h.to_string()),
                                    ("hz", &hz_label(hz)),
                                ],
                            ),
                        ));
                    }
                }
            }
            let current = mode_value(m.width, m.height, m.refresh_rate);
            if !opts.iter().any(|o| o.value == json!(current)) {
                opts.insert(0, Opt::new(json!(current), current.clone()));
            }
            with(Kind::Enum(opts), json!(current))
        }
        "scale" => {
            let mut scales = scale_options(m.width.max(1), m.height.max(1));
            if !scales.iter().any(|s| (s - m.scale).abs() < 1e-6) {
                scales.push(m.scale);
                scales.sort_by(f64::total_cmp);
            }
            with(Kind::Enum(scale_opts(&scales)), json!(m.scale))
        }
        "position" => {
            let opts = ["auto", "auto-right", "auto-left", "auto-up", "auto-down"]
                .iter()
                .map(|p| Opt::new(json!(p), t(&format!("mon.position={p}"))))
                .collect();
            with(Kind::Enum(opts), json!("auto"))
        }
        "transform" => {
            let opts = (0..4)
                .map(|n| Opt::new(json!(n), t(&format!("mon.transform={n}"))))
                .collect();
            with(Kind::Enum(opts), json!(m.transform))
        }
        _ => return None,
    })
}

fn monitors(ctx: &Ctx) -> Vec<Group> {
    // La escala general se ofrece según la primera pantalla.
    let first = ctx.monitors.first();
    let mut global: Vec<Opt> = vec![Opt::new(json!("auto"), t("o.m:scale=auto"))];
    if let Some(m) = first {
        global.extend(scale_opts(&scale_options(m.width.max(1), m.height.max(1))));
    }
    let mut groups = vec![Group::new(
        "monitors_all",
        vec![
            field("m:scale", Kind::Enum(global), json!("auto")),
            choice(
                "misc:vrr",
                &[json!(0), json!(1), json!(2), json!(3)],
                json!(0),
            ),
            toggle("xwayland:force_zero_scaling", false).advanced(),
        ],
    )];
    for m in &ctx.monitors {
        let fields = ["mode", "scale", "position", "transform"]
            .iter()
            .filter_map(|p| monitor_field(m, p))
            .collect();
        let desc = m.description.trim();
        let title = if desc.is_empty() {
            tf("mon.title", &[("name", &m.name)])
        } else {
            tf("mon.title_desc", &[("name", &m.name), ("desc", desc)])
        };
        groups.push(Group {
            id: format!("monitor:{}", m.name),
            title,
            fields,
        });
    }
    groups
}

// ---------------------------------------------------------------- dispositivos

fn keyboard() -> Vec<Group> {
    vec![
        Group::new(
            "keyboard",
            vec![
                text_presets(
                    "input:kb_layout",
                    &[
                        "es", "latam", "us", "us,es", "es,us", "latam,us", "br", "pt", "fr", "de",
                        "it", "gb",
                    ],
                    "us",
                ),
                text_presets(
                    "input:kb_variant",
                    &[
                        "",
                        "intl",
                        "deadtilde",
                        "nodeadkeys",
                        "winkeys",
                        "dvorak",
                        "colemak",
                    ],
                    "",
                ),
                choice(
                    "kbopt:grp",
                    &[
                        json!("none"),
                        json!("alt_shift_toggle"),
                        json!("win_space_toggle"),
                        json!("ctrl_shift_toggle"),
                        json!("alts_toggle"),
                        json!("caps_toggle"),
                    ],
                    json!("none"),
                ),
                choice(
                    "kbopt:compose",
                    &[
                        json!("none"),
                        json!("caps"),
                        json!("menu"),
                        json!("rwin"),
                        json!("rctrl"),
                        json!("prsc"),
                        json!("sclk"),
                        json!("ins"),
                        json!("paus"),
                        json!("ralt"),
                    ],
                    json!("caps"),
                ),
                text_presets("input:kb_model", &["", "pc105", "pc104"], "").advanced(),
            ],
        ),
        Group::new(
            "typing",
            vec![
                steps("input:repeat_rate", 10, 100, 5, 40),
                steps("input:repeat_delay", 150, 1000, 25, 250),
                toggle("input:numlock_by_default", true),
            ],
        ),
    ]
}

fn mouse() -> Vec<Group> {
    vec![
        Group::new(
            "mouse",
            vec![
                slider("input:sensitivity", -1.0, 1.0, 0.05, 0.0),
                choice(
                    "input:accel_profile",
                    &[json!(""), json!("adaptive"), json!("flat")],
                    json!(""),
                ),
                slider("input:scroll_factor", 0.2, 3.0, 0.1, 1.0),
                toggle("input:natural_scroll", false),
                toggle("input:left_handed", false),
                choice(
                    "input:follow_mouse",
                    &[json!(0), json!(1), json!(2), json!(3)],
                    json!(1),
                )
                .advanced(),
                toggle("input:mouse_refocus", true).advanced(),
            ],
        ),
        Group::new(
            "touchpad",
            vec![
                toggle("input:touchpad:natural_scroll", false),
                slider("input:touchpad:scroll_factor", 0.1, 2.0, 0.1, 0.4),
                toggle("input:touchpad:tap-to-click", true),
                toggle("input:touchpad:clickfinger_behavior", true),
                toggle("input:touchpad:disable_while_typing", true),
                toggle("x:ws_swipe", false),
                choice(
                    "input:touchpad:drag_3fg",
                    &[json!(0), json!(1), json!(2)],
                    json!(0),
                )
                .advanced(),
            ],
        ),
    ]
}

fn cursor(ctx: &Ctx) -> Vec<Group> {
    let themes = ctx
        .cursor_themes
        .iter()
        .map(|th| Opt::new(json!(th), th.clone()))
        .collect();
    vec![Group::new(
        "cursor",
        vec![
            field("x:cursor_theme", Kind::Text, json!("default")).presets(themes),
            choice(
                "x:cursor_size",
                &[
                    json!(16),
                    json!(20),
                    json!(24),
                    json!(28),
                    json!(32),
                    json!(40),
                    json!(48),
                ],
                json!(24),
            ),
            choice(
                "cursor:inactive_timeout",
                &[json!(0), json!(3), json!(5), json!(10), json!(30)],
                json!(0),
            ),
            toggle("cursor:hide_on_key_press", true),
            slider("cursor:zoom_factor", 1.0, 3.0, 0.25, 1.0),
            toggle("cursor:hide_on_touch", true).advanced(),
            toggle("cursor:zoom_rigid", false).advanced(),
            choice(
                "cursor:warp_on_change_workspace",
                &[json!(0), json!(1), json!(2)],
                json!(1),
            )
            .advanced(),
            choice(
                "cursor:no_hardware_cursors",
                &[json!(0), json!(1), json!(2)],
                json!(2),
            )
            .advanced(),
        ],
    )]
}

/// Temas de cursor instalados (carpetas de iconos con `cursors/`).
pub fn cursor_themes() -> Vec<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let dirs = [
        std::path::PathBuf::from("/usr/share/icons"),
        home.join(".local/share/icons"),
        home.join(".icons"),
    ];
    let mut out: Vec<String> = vec![];
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for e in rd.flatten() {
            if e.path().join("cursors").is_dir()
                && let Some(name) = e.file_name().to_str()
                && !out.iter().any(|x| x == name)
            {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_scales_like_omarchy() {
        assert_eq!(clean_scale(1.25, 1920, 1080), 1.25);
        // 1.6 no divide 1920x1080 en píxeles enteros.
        let s = clean_scale(1.6, 1920, 1080);
        assert!((1920.0 / s).fract() == 0.0 && (1080.0 / s).fract() == 0.0);
        assert_eq!(clean_scale(1.6, 2560, 1600), 1.6);
        assert_eq!(scale_options(1920, 1080)[0], 1.0);
    }

    #[test]
    fn parses_modes_and_keys() {
        assert_eq!(parse_mode("1920x1080@60.00Hz"), Some((1920, 1080, 60.0)));
        assert_eq!(mode_value(2560, 1440, 143.912), "2560x1440@143.91");
        assert_eq!(
            monitor_key("x:mon:HDMI-A-1:scale"),
            Some(("HDMI-A-1", "scale"))
        );
        assert_eq!(monitor_key("x:anim_speed"), None);
    }

    #[test]
    fn coerces_hyprland_values() {
        assert_eq!(coerce(&int(None, None, 1), json!(0.0)), json!(0));
        assert_eq!(coerce(&Kind::Bool, json!(1)), json!(true));
        let k = Kind::Enum(vec![Opt::new(json!(0), "a"), Opt::new(json!(5), "b")]);
        assert_eq!(coerce(&k, json!(5.0)), json!(5));
    }
}
