//! Interacción con un campo de formulario: qué pasa al activarlo (alternar,
//! abrir una lista, pedir un texto), cómo se valida lo escrito y cómo un
//! clic sobre un deslizador se convierte en valor.

use ratatui::layout::Rect;
use serde_json::{Value, json};

use crate::form::{FieldRow, input_error, input_text};
use crate::i18n::t;
use crate::popup::{
    Checklist, ColorPick, Input, InputTarget, PickItem, PickTarget, Picker, Popup, PopupTypes,
};
use crate::schema::{self, Kind, clamp_float};

/// Valor especial de la opción "Personalizado…" en las listas de sugerencias.
pub const PICK_CUSTOM: &str = "\u{0}custom";

/// Resultado de activar un campo (Enter o clic).
pub enum Activation<T: PopupTypes> {
    /// Valor nuevo inmediato (interruptores).
    Set(Value),
    Open(Popup<T>),
}

/// `anchor`: rectángulo del control, para abrir la lista pegada a él.
/// `extra_hint`: ayuda adicional para el cuadro de texto.
pub fn activate<T: PopupTypes>(
    f: &FieldRow<T::Bind>,
    anchor: Option<Rect>,
    extra_hint: Option<String>,
) -> Activation<T> {
    let def = &f.def;
    let target = || PickTarget::Field {
        bind: f.bind.clone(),
        def: Box::new(def.clone()),
    };
    match &def.kind {
        Kind::Bool => Activation::Set(Value::Bool(
            !f.effective().and_then(Value::as_bool).unwrap_or(false),
        )),
        Kind::Enum(opts) => Activation::Open(Popup::Picker(Picker {
            anchor,
            title: def.label.clone(),
            items: opts.iter().map(PickItem::from_opt).collect(),
            sel: opts
                .iter()
                .position(|o| Some(&o.value) == f.value.as_ref().or(def.default.as_ref()))
                .unwrap_or(0),
            filter: String::new(),
            current: Some(f.value.clone().unwrap_or(Value::Null)),
            target: target(),
        })),
        Kind::Multi(opts) => {
            let cur: Vec<Value> = f
                .effective()
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            Activation::Open(Popup::Checklist(Checklist {
                title: def.label.clone(),
                checked: opts.iter().map(|o| cur.contains(&o.value)).collect(),
                opts: opts.clone(),
                sel: 0,
                bind: f.bind.clone(),
            }))
        }
        Kind::Color if def.presets.is_empty() => {
            let cur = f
                .effective()
                .and_then(Value::as_str)
                .unwrap_or("#808080")
                .to_string();
            Activation::Open(Popup::Color(ColorPick::new(
                def.label.clone(),
                target(),
                &cur,
                vec![],
            )))
        }
        _ if !def.presets.is_empty() => {
            let mut items: Vec<PickItem> = def.presets.iter().map(PickItem::from_opt).collect();
            items.push(PickItem {
                label: t("pick.custom"),
                detail: t("pick.custom.desc"),
                group: String::new(),
                value: json!(PICK_CUSTOM),
                enabled: true,
            });
            let cur = f.effective().cloned();
            Activation::Open(Popup::Picker(Picker {
                anchor,
                title: def.label.clone(),
                sel: def
                    .presets
                    .iter()
                    .position(|o| Some(&o.value) == cur.as_ref())
                    .unwrap_or(items.len() - 1),
                items,
                filter: String::new(),
                current: cur,
                target: target(),
            }))
        }
        _ => Activation::Open(text_input(f, extra_hint)),
    }
}

/// Cuadro de texto para escribir el valor del campo.
pub fn text_input<T: PopupTypes>(f: &FieldRow<T::Bind>, extra_hint: Option<String>) -> Popup<T> {
    let def = &f.def;
    let mut hint = def.desc.clone();
    if matches!(def.kind, Kind::Text | Kind::Raw) {
        hint = format!(
            "{hint}\n{}",
            t(if def.kind == Kind::Raw {
                "input.raw_hint"
            } else {
                "input.text_hint"
            })
        );
    }
    if let Some(extra) = extra_hint {
        hint = format!("{hint}\n{extra}");
    }
    Popup::Input(Input::new(
        def.label.clone(),
        hint.trim().to_string(),
        &input_text(f),
        InputTarget::Field {
            bind: f.bind.clone(),
            def: Box::new(def.clone()),
        },
    ))
}

/// Lo que escribió el usuario: `Ok(None)` = volver al valor por defecto,
/// `Ok(Some(v))` = valor nuevo, `Err` = mensaje para mostrar en el cuadro.
pub fn parse_submitted(kind: &Kind, text: &str) -> Result<Option<Value>, String> {
    if text.trim().is_empty() && *kind != Kind::Bool {
        return Ok(None);
    }
    schema::parse_input(kind, text)
        .map(Some)
        .map_err(|code| input_error(kind, &code))
}

/// Valor de un deslizador cuando se hace clic o se arrastra en la columna `x`
/// de su pista `track`. `None` si el campo no es un número con límites.
pub fn slider_value(kind: &Kind, track: Rect, x: u16) -> Option<Value> {
    let Kind::Float {
        min: Some(min),
        max: Some(max),
        step,
    } = *kind
    else {
        return None;
    };
    let span = track.width.saturating_sub(1).max(1) as f64;
    let frac = (x.saturating_sub(track.x) as f64 / span).clamp(0.0, 1.0);
    let raw = min + frac * (max - min);
    let v = if step > 0.0 {
        (raw / step).round() * step
    } else {
        raw
    };
    serde_json::Number::from_f64(clamp_float(kind, v)).map(Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::float;

    #[test]
    fn slider_maps_columns_to_steps() {
        let k = float(Some(0.0), Some(1.0), 0.05);
        let track = Rect::new(10, 0, 11, 1);
        assert_eq!(slider_value(&k, track, 10), Some(json!(0.0)));
        assert_eq!(slider_value(&k, track, 20), Some(json!(1.0)));
        assert_eq!(slider_value(&k, track, 15), Some(json!(0.5)));
        assert_eq!(slider_value(&Kind::Bool, track, 15), None);
    }

    #[test]
    fn empty_text_means_default() {
        let k = schema::int(Some(0), Some(10), 1);
        assert_eq!(parse_submitted(&k, "  "), Ok(None));
        assert_eq!(parse_submitted(&k, "7"), Ok(Some(json!(7))));
        assert!(parse_submitted(&k, "70").is_err());
    }
}
