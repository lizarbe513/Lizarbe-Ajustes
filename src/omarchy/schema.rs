//! Definición unificada de campos configurables, construida desde el `schema`
//! de los manifests de Omarchy o desde el catálogo curado de esta herramienta.

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
    /// Mostrar preview de formato de fecha Qt.
    pub date_format: bool,
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
            date_format: false,
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

    pub fn date_format(mut self) -> Self {
        self.date_format = true;
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

fn str_of(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn parse_options(v: &Value) -> Vec<Opt> {
    v.get("options")
        .and_then(Value::as_array)
        .map(|opts| {
            opts.iter()
                .map(|o| match o {
                    Value::Object(_) => {
                        let value = o.get("value").cloned().unwrap_or(Value::Null);
                        let label = o
                            .get("label")
                            .and_then(Value::as_str)
                            .map(String::from)
                            .unwrap_or_else(|| value_label(&value));
                        Opt::new(value, label).with_desc(str_of(o, "description"))
                    }
                    other => Opt::new(other.clone(), value_label(other)),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Texto corto para mostrar un valor JSON.
pub fn value_label(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Convierte una entrada de `barWidget.schema` de un manifest.
pub fn from_manifest(field: &Value, defaults: Option<&Value>) -> Option<FieldDef> {
    let key = field.get("key")?.as_str()?.to_string();
    let ty = str_of(field, "type");
    let num = |k: &str| field.get(k).and_then(Value::as_f64);
    let kind = match ty.as_str() {
        "boolean" | "bool" => Kind::Bool,
        "integer" | "int" => int(
            num("min").map(|x| x as i64),
            num("max").map(|x| x as i64),
            num("step").map(|x| x as i64).unwrap_or(1),
        ),
        "number" | "float" | "real" => float(num("min"), num("max"), num("step").unwrap_or(0.1)),
        "enum" | "select" => Kind::Enum(parse_options(field)),
        "multiselect" => Kind::Multi(parse_options(field)),
        "path" | "file" | "directory" => Kind::Path,
        "color" => Kind::Color,
        _ => Kind::Text,
    };
    let label = field
        .get("label")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| key.clone());
    let default = field
        .get("defaultValue")
        .cloned()
        .or_else(|| defaults.and_then(|d| d.get(&key)).cloned());
    let mut def = FieldDef::new(&key, label, kind).desc(str_of(field, "description"));
    def.default = default;
    def.empty_text = str_of(field, "noSelectionText");
    Some(def)
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
    fn parses_manifest_fields() {
        let f = from_manifest(
            &json!({"key":"refreshIntervalSec","type":"integer","label":"Refresh","min":30,"max":3600,"step":30,"defaultValue":900}),
            None,
        )
        .unwrap();
        assert_eq!(
            f.kind,
            Kind::Int {
                min: Some(30),
                max: Some(3600),
                step: 30
            }
        );
        assert_eq!(f.default, Some(json!(900)));

        let f = from_manifest(
            &json!({"key":"syncMode","type":"enum","options":["Off","On"]}),
            Some(&json!({"syncMode":"Off"})),
        )
        .unwrap();
        assert!(matches!(f.kind, Kind::Enum(ref o) if o.len() == 2 && o[1].label == "On"));
        assert_eq!(f.default, Some(json!("Off")));
    }

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
    fn newline_escaping_roundtrip() {
        let s = "HH\n—\nmm";
        assert_eq!(escape_newlines(s), "HH\\n—\\nmm");
        assert_eq!(unescape_newlines(&escape_newlines(s)), s);
        assert_eq!(unescape_newlines("a\\\\nb"), "a\\nb");
    }
}
