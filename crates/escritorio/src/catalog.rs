//! Ajustes que ofrece Escritorio, agrupados por sección.
//!
//! Cada ajuste es una opción de Hyprland (`general:gaps_in`) o un ajuste
//! propio `x:` (ver `hyprfile`). El valor por defecto es el de Omarchy (o el
//! de Hyprland si Omarchy no lo cambia): es al que se vuelve al restaurar.
//! Los textos salen de `i18n`: `o.<clave>` (nombre), `o.<clave>.d`
//! (descripción) y `o.<clave>=<valor>` (cada opción de una lista).

use lizarbe_core::schema::{FieldDef, Kind, Opt, float, int};
use serde_json::{Value, json};

use crate::i18n::t;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Appearance,
    Windows,
    Changes,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Appearance, Section::Windows, Section::Changes];

    pub fn index(self) -> usize {
        Section::ALL.iter().position(|s| *s == self).unwrap()
    }

    /// Identificador para `--section` (en español o en inglés).
    pub fn from_id(id: &str) -> Option<Section> {
        Some(match id.trim().to_lowercase().as_str() {
            "apariencia" | "appearance" => Section::Appearance,
            "ventanas" | "windows" => Section::Windows,
            "cambios" | "changes" => Section::Changes,
            _ => return None,
        })
    }

    fn id(self) -> &'static str {
        match self {
            Section::Appearance => "appearance",
            Section::Windows => "windows",
            Section::Changes => "changes",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Section::Appearance => "󰏘",
            Section::Windows => "󰕮",
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

/// Categorías de la barra lateral: (clave de texto, secciones).
pub const CATEGORIES: [(&str, &[Section]); 2] = [
    ("cat.desktop", &[Section::Appearance, Section::Windows]),
    ("cat.review", &[Section::Changes]),
];

/// Grupo de ajustes con su encabezado.
pub struct Group {
    pub id: &'static str,
    pub fields: Vec<FieldDef>,
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

pub fn groups(section: Section) -> Vec<Group> {
    match section {
        Section::Appearance => appearance(),
        Section::Windows => windows(),
        Section::Changes => vec![],
    }
}

fn appearance() -> Vec<Group> {
    vec![
        Group {
            id: "spacing",
            fields: vec![
                steps("general:gaps_in", 0, 30, 1, 5),
                steps("general:gaps_out", 0, 50, 1, 10),
                steps("general:border_size", 0, 10, 1, 2),
                steps("decoration:rounding", 0, 30, 1, 0),
                slider("decoration:rounding_power", 1.0, 5.0, 0.25, 2.0).advanced(),
            ],
        },
        Group {
            id: "opacity",
            fields: vec![
                slider("decoration:active_opacity", 0.1, 1.0, 0.05, 1.0),
                slider("decoration:inactive_opacity", 0.1, 1.0, 0.05, 1.0),
                slider("decoration:fullscreen_opacity", 0.1, 1.0, 0.05, 1.0).advanced(),
                toggle("decoration:dim_inactive", false),
                slider("decoration:dim_strength", 0.0, 1.0, 0.05, 0.5),
                slider("decoration:dim_special", 0.0, 1.0, 0.05, 0.2).advanced(),
            ],
        },
        Group {
            id: "blur",
            fields: vec![
                toggle("decoration:blur:enabled", false),
                steps("decoration:blur:size", 1, 20, 1, 8),
                steps("decoration:blur:passes", 1, 6, 1, 1),
                slider("decoration:blur:vibrancy", 0.0, 1.0, 0.05, 0.17).advanced(),
                toggle("decoration:blur:xray", false).advanced(),
                toggle("decoration:blur:ignore_opacity", true).advanced(),
                toggle("decoration:blur:new_optimizations", true).advanced(),
            ],
        },
        Group {
            id: "shadow",
            fields: vec![
                toggle("decoration:shadow:enabled", false),
                steps("decoration:shadow:range", 1, 40, 1, 4),
                steps("decoration:shadow:render_power", 1, 4, 1, 3).advanced(),
                toggle("decoration:shadow:sharp", false).advanced(),
            ],
        },
        Group {
            id: "animations",
            fields: vec![
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
        },
    ]
}

fn windows() -> Vec<Group> {
    vec![
        Group {
            id: "layout",
            fields: vec![
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
        },
        Group {
            id: "moving",
            fields: vec![
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
        },
        Group {
            id: "workspaces",
            fields: vec![
                steps("x:ws_persistent", 0, 10, 1, 0),
                toggle("binds:workspace_back_and_forth", false),
                toggle("binds:allow_workspace_cycles", false).advanced(),
                toggle("binds:hide_special_on_workspace_change", true).advanced(),
            ],
        },
        Group {
            id: "focus",
            fields: vec![
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
        },
        Group {
            id: "groups",
            fields: vec![
                toggle("group:groupbar:enabled", true),
                steps("group:groupbar:font_size", 8, 20, 1, 12).advanced(),
                steps("group:groupbar:height", 14, 36, 2, 22).advanced(),
                toggle("group:groupbar:gradients", true).advanced(),
            ],
        },
    ]
}

/// Todas las claves de opciones de Hyprland del catálogo (sin las `x:`).
pub fn hypr_keys() -> Vec<String> {
    Section::ALL
        .iter()
        .flat_map(|s| groups(*s))
        .flat_map(|g| g.fields)
        .map(|f| f.key)
        .filter(|k| !k.starts_with("x:"))
        .collect()
}

/// Definición de un ajuste por su clave.
pub fn find(key: &str) -> Option<FieldDef> {
    Section::ALL
        .iter()
        .flat_map(|s| groups(*s))
        .flat_map(|g| g.fields)
        .find(|f| f.key == key)
}
