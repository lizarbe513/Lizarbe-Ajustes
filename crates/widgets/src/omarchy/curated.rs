//! Catálogo curado de ajustes.
//!
//! Muchos widgets de Omarchy leen ajustes (`setting("format")`, …) que su
//! manifest no declara. Aquí se describen para poder ofrecer formularios
//! completos, con textos en el idioma del usuario y valores sugeridos.

use serde_json::{Value, json};

use super::catalog::{Catalog, Plugin};
use super::qt_format;
use super::schema::{FieldDef, Kind, Opt, float, int};
use super::shell_json as sj;
use crate::i18n::{t, t_manifest};

/// Formatos que recorre el reloj al hacer clic derecho (panels/clock/Model.js).
pub const CLOCK_FORMATS: [&str; 8] = [
    "dddd HH:mm",
    "dddd h:mm AP",
    "HH:mm",
    "h:mm AP",
    "ddd d MMM HH:mm",
    "ddd d MMM h:mm AP",
    "d MMMM 'W'ww yyyy",
    "yyyy-MM-dd HH:mm",
];

pub const VERTICAL_CLOCK_FORMATS: [&str; 4] = [
    "HH\n—\nmm",
    "h\n—\nmm\nAP",
    "dd\nMMM\n'W'ww\n''yy",
    "HH\nmm",
];

fn date_presets(list: &[&str]) -> Vec<Opt> {
    list.iter()
        .map(|f| Opt::new(json!(f), qt_format::preview(f)).with_desc(f.replace('\n', "\\n")))
        .collect()
}

fn field(key: &str, prefix: &str, kind: Kind) -> FieldDef {
    FieldDef::new(key, t(&format!("{prefix}.{key}")), kind).desc(t(&format!("{prefix}.{key}.desc")))
}

/// Campos curados para widgets sin `schema` en su manifest.
fn curated_widget_fields(id: &str, entry: &Value) -> Vec<FieldDef> {
    let ty = entry.get("type").and_then(Value::as_str).unwrap_or("");
    // Un clon (`usuario.clock`) acepta los mismos ajustes que el original.
    let base = id.rsplit('.').next().unwrap_or(id);
    let p = "cw";
    match (ty, base) {
        ("command", _) => vec![
            field("exec", p, Kind::Text),
            field("interval", p, int(Some(1), Some(86_400), 1)).default(json!(5)),
            field("text", p, Kind::Text),
            field("tooltip", p, Kind::Text),
            field("onClick", p, Kind::Text),
            field("onRightClick", p, Kind::Text).advanced(),
            field("onMiddleClick", p, Kind::Text).advanced(),
            field("keepSpace", p, Kind::Bool)
                .default(json!(false))
                .advanced(),
            field("fontSize", p, int(Some(6), Some(48), 1))
                .default(json!(12))
                .advanced(),
            field("horizontalMargin", p, float(Some(0.0), Some(64.0), 0.5))
                .default(json!(7.5))
                .advanced(),
            field("verticalPadding", p, int(Some(0), Some(64), 1))
                .default(json!(6))
                .advanced(),
        ],
        ("qml", _) => vec![field("source", p, Kind::Path)],
        (_, "clock") => vec![
            field("format", p, Kind::Text)
                .default(json!("dddd HH:mm"))
                .presets(date_presets(&CLOCK_FORMATS))
                .preview(super::qt_format::preview),
            field("formatAlt", p, Kind::Text)
                .default(json!("d MMMM 'W'ww yyyy"))
                .presets(date_presets(&CLOCK_FORMATS))
                .preview(super::qt_format::preview),
            field("verticalFormat", p, Kind::Text)
                .default(json!("HH\n—\nmm"))
                .presets(date_presets(&VERTICAL_CLOCK_FORMATS))
                .preview(super::qt_format::preview),
            field("verticalFormatAlt", p, Kind::Text)
                .default(json!("dd\nMMM\n'W'ww\n''yy"))
                .presets(date_presets(&VERTICAL_CLOCK_FORMATS))
                .preview(super::qt_format::preview)
                .advanced(),
            field(
                "weekStartDay",
                p,
                Kind::Enum(
                    std::iter::once(Opt::new(Value::Null, t("opt.auto")))
                        .chain(
                            [
                                "monday",
                                "tuesday",
                                "wednesday",
                                "thursday",
                                "friday",
                                "saturday",
                                "sunday",
                            ]
                            .iter()
                            .map(|d| Opt::new(json!(d), t(&format!("day.{d}")))),
                        )
                        .collect(),
                ),
            ),
            field("birthYear", p, int(Some(1900), Some(2100), 1)).advanced(),
            field("lifeExpectancy", p, int(Some(1), Some(150), 1))
                .default(json!(90))
                .advanced(),
        ],
        (_, "weather") => vec![
            field(
                "unit",
                p,
                Kind::Enum(vec![
                    Opt::new(Value::Null, t("opt.auto")),
                    Opt::new(json!("metric"), t("opt.metric")),
                    Opt::new(json!("imperial"), t("opt.imperial")),
                ]),
            ),
            field("refreshMinutes", p, int(Some(1), Some(1440), 5)).default(json!(15)),
        ],
        (_, "spacer") => {
            vec![field("size", p, int(Some(0), Some(400), 2)).default(json!(12))]
        }
        (_, "power") => vec![field("showPercentage", p, Kind::Bool).default(json!(false))],
        (_, "active-window") => {
            vec![field("maxWidth", p, int(Some(40), Some(2000), 10)).default(json!(280))]
        }
        _ => vec![],
    }
}

/// Traduce (si hay traducción) un campo que viene de un manifest.
fn localize_manifest_field(id: &str, mut f: FieldDef) -> FieldDef {
    let base = id.rsplit('.').next().unwrap_or(id);
    let k = format!("{base}.{}", f.key);
    if let Some(l) = t_manifest(&format!("{k}.label")) {
        f.label = l;
    }
    if let Some(d) = t_manifest(&format!("{k}.desc")) {
        f.desc = d;
    }
    if let Some(e) = t_manifest(&format!("{k}.empty")) {
        f.empty_text = e;
    }
    if let Kind::Enum(opts) | Kind::Multi(opts) = &mut f.kind {
        for o in opts.iter_mut() {
            let v = super::schema::value_label(&o.value);
            if let Some(l) = t_manifest(&format!("{k}.opt.{v}")) {
                o.label = l;
            }
            if let Some(d) = t_manifest(&format!("{k}.opt.{v}.desc")) {
                o.desc = d;
            }
        }
    }
    f
}

/// Todos los campos editables de una entrada de la barra.
pub fn widget_fields(id: &str, entry: &Value, plugin: Option<&Plugin>) -> Vec<FieldDef> {
    let mut fields: Vec<FieldDef> = plugin
        .and_then(|p| p.bar_widget.as_ref())
        .map(|b| {
            b.schema
                .iter()
                .cloned()
                .map(|f| localize_manifest_field(id, f))
                .collect()
        })
        .unwrap_or_default();
    for f in curated_widget_fields(id, entry) {
        if !fields.iter().any(|x| x.key == f.key) {
            fields.push(f);
        }
    }
    fields
}

/// Nombre del widget en el idioma de la interfaz.
pub fn widget_name(id: &str, entry: Option<&Value>, catalog: &Catalog) -> String {
    let base = id.rsplit('.').next().unwrap_or(id);
    if let Some(e) = entry
        && let Some(ty) = e.get("type").and_then(Value::as_str)
        && (ty == "command" || ty == "qml")
    {
        return id.to_string();
    }
    match catalog.get(id) {
        Some(p) if !p.first_party && p.cloned_from.is_empty() => p.display_name(),
        Some(p) => {
            let name = t_manifest(&format!("{base}.name")).unwrap_or_else(|| p.display_name());
            if p.cloned_from.is_empty() {
                name
            } else {
                format!("{name} ({})", t("label.clone"))
            }
        }
        None => id.to_string(),
    }
}

pub fn widget_description(plugin: &Plugin) -> String {
    let base = plugin.id.rsplit('.').next().unwrap_or(&plugin.id);
    let ours = plugin.first_party || !plugin.cloned_from.is_empty();
    ours.then(|| t_manifest(&format!("{base}.desc")))
        .flatten()
        .unwrap_or_else(|| plugin.display_description())
}

pub fn category_name(cat: &str) -> String {
    t_manifest(&format!("category.{cat}")).unwrap_or_else(|| cat.to_string())
}

/// Ajustes generales de la barra.
pub fn bar_fields(cfg: &Value, catalog: &Catalog) -> Vec<FieldDef> {
    let p = "bar";
    let mut anchor_opts = vec![Opt::new(json!(""), t("opt.none"))];
    for e in sj::section(cfg, 1) {
        let id = sj::entry_id(e);
        if !anchor_opts.iter().any(|o| o.value == json!(id)) {
            anchor_opts.push(Opt::new(json!(id), widget_name(&id, Some(e), catalog)));
        }
    }
    let mut bar_opts = vec![Opt::new(Value::Null, t("bar.builtin"))];
    for b in catalog.bar_options() {
        if b.id != "omarchy.bar" {
            bar_opts.push(Opt::new(json!(b.id), b.display_name()).with_desc(b.description.clone()));
        }
    }
    vec![
        field(
            "position",
            p,
            Kind::Enum(
                ["top", "bottom", "left", "right"]
                    .iter()
                    .map(|x| Opt::new(json!(x), t(&format!("pos.{x}"))))
                    .collect(),
            ),
        )
        .default(json!("top")),
        field("transparent", p, Kind::Bool).default(json!(false)),
        field("centerAnchor", p, Kind::Enum(anchor_opts)).default(json!("")),
        field("id", p, Kind::Enum(bar_opts)).advanced(),
    ]
}

/// Tiempos de inactividad (segundos).
pub fn idle_fields() -> Vec<FieldDef> {
    let presets = |list: &[i64]| -> Vec<Opt> {
        list.iter()
            .map(|s| Opt::new(json!(s), human_seconds(*s)))
            .collect()
    };
    let list = [30, 60, 120, 150, 180, 300, 600, 900, 1200, 1800, 3600];
    vec![
        field("screensaver", "idle", int(Some(5), Some(86_400), 30))
            .default(json!(150))
            .presets(presets(&list))
            .seconds(),
        field("lock", "idle", int(Some(5), Some(86_400), 30))
            .default(json!(300))
            .presets(presets(&list))
            .seconds(),
    ]
}

pub use lizarbe_core::schema::human_seconds;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_has_format_fields() {
        let f = widget_fields("omarchy.clock", &json!({"id": "omarchy.clock"}), None);
        assert!(f.iter().any(|x| x.key == "format" && x.preview.is_some()));
        // Un clon también.
        let f = widget_fields("leo.clock", &json!({"id": "leo.clock"}), None);
        assert!(f.iter().any(|x| x.key == "formatAlt"));
    }

    /// Las claves construidas con `format!` existen en ambos idiomas.
    #[test]
    fn dynamic_keys_are_translated() {
        use crate::i18n::{Lang, set_lang, t_opt};
        for lang in [Lang::Es, Lang::En] {
            set_lang(lang);
            let mut fields = vec![];
            for (id, entry) in [
                ("omarchy.clock", json!({})),
                ("omarchy.weather", json!({})),
                ("omarchy.spacer", json!({})),
                ("omarchy.power", json!({})),
                ("omarchy.active-window", json!({})),
                ("x", json!({"type": "command"})),
                ("y", json!({"type": "qml"})),
            ] {
                fields.extend(widget_fields(id, &entry, None));
            }
            fields.extend(bar_fields(&json!({}), &Catalog::default()));
            fields.extend(idle_fields());
            for f in &fields {
                assert!(!f.label.contains('.'), "sin traducir: {}", f.label);
                assert!(
                    !f.desc.starts_with("cw.")
                        && !f.desc.starts_with("bar.")
                        && !f.desc.starts_with("idle."),
                    "sin traducir: {}",
                    f.desc
                );
                if let Kind::Enum(opts) = &f.kind {
                    assert!(
                        opts.iter().all(|o| !o.label.contains('.')),
                        "opción sin traducir en {}",
                        f.key
                    );
                }
            }
            for (s, k) in super::super::shell_toml::SIMPLE_KEYS {
                assert!(t_opt(&format!("ap.{s}.{k}")).is_some());
                assert!(t_opt(&format!("ap.{s}.{k}.desc")).is_some());
            }
            for s in ["left", "center", "right"] {
                assert!(t_opt(&format!("col.{s}")).is_some());
            }
        }
    }

    #[test]
    fn command_module_fields() {
        let f = widget_fields("vpn", &json!({"id": "vpn", "type": "command"}), None);
        assert!(f.iter().any(|x| x.key == "exec"));
    }
}
