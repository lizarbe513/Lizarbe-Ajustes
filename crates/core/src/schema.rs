//! Definición de campos configurables (tipo, límites, opciones, valor por
//! defecto) y validación de lo que escribe el usuario.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Opt {
    /// Valor guardado. `Value::Null` significa "sin valor" (se borra la clave).
    pub value: Value,
    pub label: String,
    pub desc: String,
}

impl Opt {
    pub fn new(value: Value, label: impl Into<String>) -> Self {
        Opt {
            value,
            label: label.into(),
            desc: String::new(),
        }
    }

    pub fn with_desc(mut self, desc: impl Into<String>) -> Self {
        self.desc = desc.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Bool,
    Int {
        min: Option<i64>,
        max: Option<i64>,
        step: i64,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
        step: f64,
    },
    Enum(Vec<Opt>),
    Multi(Vec<Opt>),
    Text,
    Path,
    Color,
    /// Valor JSON libre (editor de claves crudas del modo avanzado).
    Raw,
}

/// Función que muestra cómo queda un texto (p. ej. un formato de fecha).
#[derive(Clone, Copy)]
pub struct Preview(pub fn(&str) -> String);

impl Preview {
    pub fn show(&self, s: &str) -> String {
        (self.0)(s)
    }
}

impl PartialEq for Preview {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.0, other.0)
    }
}

impl std::fmt::Debug for Preview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Preview")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    pub desc: String,
    pub kind: Kind,
    /// Valor que usa el shell cuando la clave no existe.
    pub default: Option<Value>,
    /// Solo se muestra en modo avanzado.
    pub advanced: bool,
    /// Valores sugeridos (formatos de reloj, tiempos de inactividad, …).
    pub presets: Vec<Opt>,
    /// Texto vacío en `multiselect` (p.ej. "Todos los indicadores").
    pub empty_text: String,
    /// Vista previa del texto (p. ej. cómo queda un formato de fecha).
    pub preview: Option<Preview>,
    /// El número son segundos (se muestra como "2 min 30 s").
    pub seconds: bool,
    /// Valor de ejemplo cuando no hay valor ni predeterminado.
    pub example: Option<Value>,
}

impl FieldDef {
    pub fn new(key: &str, label: impl Into<String>, kind: Kind) -> Self {
        FieldDef {
            key: key.to_string(),
            label: label.into(),
            desc: String::new(),
            kind,
            default: None,
            advanced: false,
            presets: vec![],
            empty_text: String::new(),
            preview: None,
            seconds: false,
            example: None,
        }
    }

    pub fn desc(mut self, d: impl Into<String>) -> Self {
        self.desc = d.into();
        self
    }

    pub fn default(mut self, v: Value) -> Self {
        self.default = Some(v);
        self
    }

    pub fn advanced(mut self) -> Self {
        self.advanced = true;
        self
    }

    pub fn presets(mut self, p: Vec<Opt>) -> Self {
        self.presets = p;
        self
    }

    pub fn preview(mut self, f: fn(&str) -> String) -> Self {
        self.preview = Some(Preview(f));
        self
    }

    pub fn seconds(mut self) -> Self {
        self.seconds = true;
        self
    }
}

pub fn int(min: Option<i64>, max: Option<i64>, step: i64) -> Kind {
    Kind::Int {
        min,
        max,
        step: step.max(1),
    }
}

pub fn float(min: Option<f64>, max: Option<f64>, step: f64) -> Kind {
    Kind::Float { min, max, step }
}

/// Texto corto para mostrar un valor JSON.
pub fn value_label(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Ajusta un número a los límites y paso del campo.
pub fn clamp_int(kind: &Kind, n: i64) -> i64 {
    if let Kind::Int { min, max, .. } = kind {
        let mut n = n;
        if let Some(m) = min {
            n = n.max(*m);
        }
        if let Some(m) = max {
            n = n.min(*m);
        }
        n
    } else {
        n
    }
}

pub fn clamp_float(kind: &Kind, n: f64) -> f64 {
    if let Kind::Float { min, max, .. } = kind {
        let mut n = n;
        if let Some(m) = min {
            n = n.max(*m);
        }
        if let Some(m) = max {
            n = n.min(*m);
        }
        // Evita 0.30000000000000004 en la configuración.
        (n * 1000.0).round() / 1000.0
    } else {
        n
    }
}

/// Valida y convierte texto escrito por el usuario al tipo del campo.
pub fn parse_input(kind: &Kind, text: &str) -> Result<Value, String> {
    let t = text.trim();
    match kind {
        Kind::Int { min, max, .. } => {
            let n: i64 = t.parse().map_err(|_| "int".to_string())?;
            if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                return Err("range".into());
            }
            Ok(Value::from(n))
        }
        Kind::Float { min, max, .. } => {
            let n: f64 = t
                .replace(',', ".")
                .parse()
                .map_err(|_| "float".to_string())?;
            if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                return Err("range".into());
            }
            Ok(serde_json::Number::from_f64(n)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }
        Kind::Bool => match t.to_lowercase().as_str() {
            "true" | "1" | "yes" | "si" | "sí" => Ok(Value::Bool(true)),
            "false" | "0" | "no" => Ok(Value::Bool(false)),
            _ => Err("bool".into()),
        },
        Kind::Color => {
            if t.starts_with('#') && !is_hex_color(t) {
                return Err("color".into());
            }
            Ok(Value::String(t.to_string()))
        }
        Kind::Raw => {
            Ok(serde_json::from_str(t).unwrap_or_else(|_| Value::String(text.to_string())))
        }
        // El texto se guarda tal cual, salvo las secuencias "\n" que permiten
        // escribir formatos de varias líneas en un campo de una sola línea.
        _ => Ok(Value::String(unescape_newlines(text))),
    }
}

pub fn is_hex_color(s: &str) -> bool {
    let h = s.trim_start_matches('#');
    s.starts_with('#') && matches!(h.len(), 3 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn escape_newlines(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

/// "2 min 30 s", "1 h", …
pub fn human_seconds(s: i64) -> String {
    if s <= 0 {
        return crate::i18n::t("time.immediate");
    }
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    let mut parts = vec![];
    if h > 0 {
        parts.push(format!("{h} h"));
    }
    if m > 0 {
        parts.push(format!("{m} min"));
    }
    if sec > 0 {
        parts.push(format!("{sec} s"));
    }
    parts.join(" ")
}

pub fn unescape_newlines(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn input_validation() {
        let k = int(Some(1), Some(10), 1);
        assert_eq!(parse_input(&k, " 5 "), Ok(json!(5)));
        assert!(parse_input(&k, "11").is_err());
        assert!(parse_input(&k, "abc").is_err());
        assert_eq!(
            parse_input(&float(Some(0.0), Some(1.0), 0.05), "0,5"),
            Ok(json!(0.5))
        );
        assert!(parse_input(&Kind::Color, "#12345").is_err());
        assert_eq!(parse_input(&Kind::Color, "#a1b2c3"), Ok(json!("#a1b2c3")));
    }

    #[test]
    fn humanizes_seconds() {
        assert_eq!(human_seconds(150), "2 min 30 s");
        assert_eq!(human_seconds(3600), "1 h");
        assert_eq!(human_seconds(45), "45 s");
    }

    #[test]
    fn newline_escaping_roundtrip() {
        let s = "HH\n—\nmm";
        assert_eq!(escape_newlines(s), "HH\\n—\\nmm");
        assert_eq!(unescape_newlines(&escape_newlines(s)), s);
        assert_eq!(unescape_newlines("a\\\\nb"), "a\\nb");
    }
}
