//! Formularios genéricos: filas (encabezados, notas, campos, acciones) y la
//! lógica para mostrar y modificar valores de campos.

use serde_json::Value;

use crate::i18n::t;
use crate::omarchy::curated::human_seconds;
use crate::omarchy::qt_format;
use crate::omarchy::schema::{self, FieldDef, Kind, clamp_float, clamp_int, value_label};
use crate::store::{Bind, Store};

#[derive(Debug, Clone, PartialEq)]
pub enum NoteKind {
    Info,
    Warn,
}

/// Acciones disparadas desde filas tipo botón.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    AddRawKey(Bind),
    CreateQmlTemplate(String),
    OpenEditor(std::path::PathBuf),
    RestartShell,
}

#[derive(Debug, Clone)]
pub struct FieldRow {
    pub def: FieldDef,
    pub bind: Bind,
    pub value: Option<Value>,
}

impl FieldRow {
    pub fn new(def: FieldDef, bind: Bind, store: &Store) -> Self {
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
pub enum Row {
    Header(String),
    Note(String, NoteKind),
    Field(FieldRow),
    Action(String, Act),
}

impl Row {
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
    pub fn clamp(&mut self, rows: &[Row]) {
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

    pub fn step(&mut self, rows: &[Row], down: bool) {
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

    pub fn first(&mut self, rows: &[Row]) {
        self.sel = 0;
        self.clamp(rows);
    }

    pub fn last(&mut self, rows: &[Row]) {
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
pub fn display_value(f: &FieldRow) -> String {
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
        Kind::Text | Kind::Path | Kind::Color => match v.and_then(Value::as_str) {
            None => "—".into(),
            Some("") => format!("({})", t("val.empty")),
            Some(s) if def.date_format => {
                format!(
                    "{}   → {}",
                    schema::escape_newlines(s),
                    qt_format::preview(s)
                )
            }
            Some(s) => schema::escape_newlines(s),
        },
        Kind::Raw => v.map(|x| x.to_string()).unwrap_or_else(|| "—".into()),
    }
}

/// `true` si el valor mostrado es el predeterminado (la clave no existe).
pub fn is_default(f: &FieldRow) -> bool {
    f.value.is_none()
}

/// Cambia el valor con ←/→: alterna, recorre opciones o suma el paso.
pub fn nudge(store: &mut Store, f: &FieldRow, forward: bool) {
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
pub fn input_text(f: &FieldRow) -> String {
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
        ("range", Kind::Int { min, max, .. }) => crate::i18n::tf(
            "err.range",
            &[
                ("min", &min.map(|x| x.to_string()).unwrap_or("-∞".into())),
                ("max", &max.map(|x| x.to_string()).unwrap_or("∞".into())),
            ],
        ),
        ("range", Kind::Float { min, max, .. }) => crate::i18n::tf(
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
