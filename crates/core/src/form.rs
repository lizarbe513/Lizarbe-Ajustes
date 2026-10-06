//! Formularios genéricos: filas (encabezados, notas, campos, acciones) y la
//! lógica para mostrar y modificar valores de campos.
//!
//! Cada aplicación decide dónde vive cada valor con su tipo de enlace `B`
//! (una ruta en un JSON, una opción de Hyprland…) e implementa
//! [`FieldStore`] para leerlo y cambiarlo. `A` son sus acciones de botón.

use serde_json::Value;

use crate::i18n::{t, tf};
use crate::schema::{self, FieldDef, Kind, clamp_float, clamp_int, human_seconds, value_label};

/// Acceso a los valores de los campos (con cambios pendientes).
pub trait FieldStore<B> {
    /// Valor actual, incluyendo cambios sin aplicar; `None` = sin fijar.
    fn get(&self, bind: &B) -> Option<Value>;
    /// Valor guardado en disco, para marcar los campos cambiados.
    fn get_original(&self, bind: &B) -> Option<Value>;
    /// Fija un valor; igualarlo a `default` puede equivaler a quitarlo.
    fn set_field(&mut self, bind: &B, value: Value, default: Option<&Value>);
    /// Vuelve al valor por defecto.
    fn unset(&mut self, bind: &B);
}

#[derive(Debug, Clone, PartialEq)]
pub enum NoteKind {
    Info,
    Warn,
}

#[derive(Debug, Clone)]
pub struct FieldRow<B> {
    pub def: FieldDef,
    pub bind: B,
    pub value: Option<Value>,
}

impl<B> FieldRow<B> {
    pub fn new(def: FieldDef, bind: B, store: &impl FieldStore<B>) -> Self {
        let value = store.get(&bind);
        FieldRow { def, bind, value }
    }

    /// Valor efectivo: el explícito o el de por defecto.
    pub fn effective(&self) -> Option<&Value> {
        self.value.as_ref().or(self.def.default.as_ref())
    }
}

// Las filas se construyen en cada fotograma y son pocas: no compensa
// encajonar `FieldRow`.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Row<B, A> {
    Header(String),
    Note(String, NoteKind),
    Field(FieldRow<B>),
    Action(String, A),
}

impl<B, A> Row<B, A> {
    pub fn selectable(&self) -> bool {
        matches!(self, Row::Field(_) | Row::Action(..))
    }
}

#[derive(Debug, Clone, Default)]
pub struct FormState {
    pub sel: usize,
    pub offset: usize,
}

impl FormState {
    /// Ajusta la selección a una fila seleccionable.
    pub fn clamp<B, A>(&mut self, rows: &[Row<B, A>]) {
        if rows.is_empty() {
            self.sel = 0;
            return;
        }
        self.sel = self.sel.min(rows.len() - 1);
        if !rows[self.sel].selectable() {
            if let Some(i) = (self.sel..rows.len()).find(|&i| rows[i].selectable()) {
                self.sel = i;
            } else if let Some(i) = (0..self.sel).rev().find(|&i| rows[i].selectable()) {
                self.sel = i;
            }
        }
    }

    pub fn step<B, A>(&mut self, rows: &[Row<B, A>], down: bool) {
        let n = rows.len();
        if n == 0 {
            return;
        }
        let mut i = self.sel;
        loop {
            if down {
                if i + 1 >= n {
                    return;
                }
                i += 1;
            } else {
                if i == 0 {
                    return;
                }
                i -= 1;
            }
            if rows[i].selectable() {
                self.sel = i;
                return;
            }
        }
    }

    pub fn first<B, A>(&mut self, rows: &[Row<B, A>]) {
        self.sel = 0;
        self.clamp(rows);
    }

    pub fn last<B, A>(&mut self, rows: &[Row<B, A>]) {
        self.sel = rows.len().saturating_sub(1);
        if let Some(i) = (0..rows.len()).rev().find(|&i| rows[i].selectable()) {
            self.sel = i;
        }
    }
}

fn option_label(opts: &[schema::Opt], v: Option<&Value>) -> Option<String> {
    let v = v.unwrap_or(&Value::Null);
    opts.iter().find(|o| &o.value == v).map(|o| o.label.clone())
}

/// Texto del valor tal como se muestra en la fila.
pub fn display_value<B>(f: &FieldRow<B>) -> String {
    if f.effective().is_none()
        && let Some(ex) = &f.def.example
    {
        return format!("— ({} {})", t("val.example"), value_label(ex));
    }
    let v = f.effective();
    let def = &f.def;
    match &def.kind {
        Kind::Bool => {
            if v.and_then(Value::as_bool).unwrap_or(false) {
                format!("■ {}", t("val.on"))
            } else {
                format!("□ {}", t("val.off"))
            }
        }
        Kind::Enum(opts) => option_label(opts, f.value.as_ref())
            .or_else(|| f.value.as_ref().map(value_label))
            .or_else(|| option_label(opts, def.default.as_ref()))
            .unwrap_or_else(|| t("val.auto")),
        Kind::Multi(opts) => {
            let sel: Vec<String> = v
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|x| option_label(opts, Some(x)).unwrap_or_else(|| value_label(x)))
                        .collect()
                })
                .unwrap_or_default();
            if sel.is_empty() {
                if def.empty_text.is_empty() {
                    t("val.all")
                } else {
                    def.empty_text.clone()
                }
            } else {
                sel.join(", ")
            }
        }
        Kind::Int { .. } | Kind::Float { .. } => match v {
            None => "—".into(),
            Some(n) if def.seconds => n
                .as_i64()
                .map(human_seconds)
                .unwrap_or_else(|| n.to_string()),
            Some(n) => n.to_string(),
        },
        // Texto con sugerencias: el nombre de la sugerencia si coincide.
        Kind::Text if def.preview.is_none() && option_label(&def.presets, v).is_some() => {
            option_label(&def.presets, v).unwrap_or_default()
        }
        Kind::Text | Kind::Path | Kind::Color => match v.and_then(Value::as_str) {
            None => "—".into(),
            Some("") => format!("({})", t("val.empty")),
            Some(s) if def.preview.is_some() => {
                let preview = def.preview.map(|p| p.show(s)).unwrap_or_default();
                format!("{}   → {preview}", schema::escape_newlines(s))
            }
            Some(s) => schema::escape_newlines(s),
        },
        Kind::Raw => v.map(|x| x.to_string()).unwrap_or_else(|| "—".into()),
    }
}

/// Cambia el valor con ←/→: alterna, recorre opciones o suma el paso.
pub fn nudge<B>(store: &mut impl FieldStore<B>, f: &FieldRow<B>, forward: bool) {
    let def = &f.def;
    let cycle = |opts: &[schema::Opt], cur: Option<&Value>| -> Option<Value> {
        if opts.is_empty() {
            return None;
        }
        let cur = cur.cloned().unwrap_or(Value::Null);
        let pos = opts.iter().position(|o| o.value == cur);
        let next = match (pos, forward) {
            (Some(i), true) => (i + 1) % opts.len(),
            (Some(i), false) => (i + opts.len() - 1) % opts.len(),
            (None, _) => 0,
        };
        Some(opts[next].value.clone())
    };
    let new = match &def.kind {
        Kind::Bool => Some(Value::Bool(
            !f.effective().and_then(Value::as_bool).unwrap_or(false),
        )),
        Kind::Enum(opts) => {
            let cur = f.value.as_ref().or(def.default.as_ref());
            cycle(opts, cur)
        }
        _ if !def.presets.is_empty() => cycle(&def.presets, f.effective()),
        Kind::Int { step, min, .. } => {
            let cur = f
                .effective()
                .or(def.example.as_ref())
                .and_then(Value::as_i64)
                .unwrap_or(min.unwrap_or(0));
            let n = if forward { cur + step } else { cur - step };
            Some(Value::from(clamp_int(&def.kind, n)))
        }
        Kind::Float { step, min, .. } => {
            let cur = f
                .effective()
                .or(def.example.as_ref())
                .and_then(Value::as_f64)
                .unwrap_or(min.unwrap_or(0.0));
            let n = if forward { cur + step } else { cur - step };
            serde_json::Number::from_f64(clamp_float(&def.kind, n)).map(Value::Number)
        }
        _ => None,
    };
    if let Some(v) = new {
        store.set_field(&f.bind, v, def.default.as_ref());
    }
}

/// Texto inicial del editor de texto para un campo.
pub fn input_text<B>(f: &FieldRow<B>) -> String {
    match (&f.def.kind, f.effective()) {
        (Kind::Raw, Some(v)) => v.to_string(),
        (_, Some(Value::String(s))) => schema::escape_newlines(s),
        (_, Some(Value::Null)) | (_, None) => String::new(),
        (_, Some(v)) => v.to_string(),
    }
}

/// Mensaje para un error de `schema::parse_input`.
pub fn input_error(kind: &Kind, code: &str) -> String {
    match (code, kind) {
        ("range", Kind::Int { min, max, .. }) => tf(
            "err.range",
            &[
                ("min", &min.map(|x| x.to_string()).unwrap_or("-∞".into())),
                ("max", &max.map(|x| x.to_string()).unwrap_or("∞".into())),
            ],
        ),
        ("range", Kind::Float { min, max, .. }) => tf(
            "err.range",
            &[
                ("min", &min.map(|x| x.to_string()).unwrap_or("-∞".into())),
                ("max", &max.map(|x| x.to_string()).unwrap_or("∞".into())),
            ],
        ),
        ("int", _) => t("err.int"),
        ("float", _) => t("err.float"),
        ("color", _) => t("err.color"),
        ("bool", _) => t("err.bool"),
        _ => code.to_string(),
    }
}
