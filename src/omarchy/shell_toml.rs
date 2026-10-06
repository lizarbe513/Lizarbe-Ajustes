//! Apariencia del shell: `~/.config/omarchy/shell.toml` es una capa del
//! usuario sobre `current/theme/shell.toml`. Las claves del usuario ganan.

use serde_json::Value;
use toml_edit::{DocumentMut, Item, Table};

use super::schema::{FieldDef, Kind, float, int, is_hex_color};

/// Una clave disponible en el `shell.toml` del tema.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeKey {
    pub section: String,
    pub key: String,
    /// Valor del tema; `None` si en el tema solo aparece comentada.
    pub value: Option<Value>,
    /// Valor de ejemplo cuando la clave está comentada en el tema.
    pub example: Option<Value>,
    /// Comentarios que la preceden en el tema (documentación en inglés).
    pub doc: String,
}

#[derive(Debug, Clone, Default)]
pub struct ThemeToml {
    pub keys: Vec<ThemeKey>,
}

impl ThemeToml {
    pub fn parse(text: &str) -> ThemeToml {
        let mut keys = vec![];
        let mut section = String::new();
        let mut doc: Vec<String> = vec![];
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() {
                doc.clear();
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line.trim_matches(['[', ']']).trim().to_string();
                doc.clear();
                continue;
            }
            let (commented, body) = match line.strip_prefix('#') {
                Some(rest) => (true, rest.trim()),
                None => (false, line),
            };
            if let Some((k, v)) = body.split_once('=') {
                let k = k.trim();
                let valid_key = !k.is_empty()
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                let value = v
                    .trim()
                    .parse::<toml_edit::Value>()
                    .ok()
                    .map(|v| toml_to_json(&v));
                if !section.is_empty() && valid_key && value.is_some() {
                    keys.retain(|x: &ThemeKey| !(x.section == section && x.key == k));
                    keys.push(ThemeKey {
                        section: section.clone(),
                        key: k.to_string(),
                        value: if commented { None } else { value.clone() },
                        example: if commented { value } else { None },
                        doc: doc.join(" "),
                    });
                    if !commented {
                        doc.clear();
                    }
                    continue;
                }
            }
            if commented {
                doc.push(body.to_string());
            }
        }
        ThemeToml { keys }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&ThemeKey> {
        self.keys
            .iter()
            .find(|k| k.section == section && k.key == key)
    }

    pub fn sections(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for k in &self.keys {
            if !out.contains(&k.section) {
                out.push(k.section.clone());
            }
        }
        out
    }
}

pub fn toml_to_json(v: &toml_edit::Value) -> Value {
    match v {
        toml_edit::Value::String(s) => Value::String(s.value().clone()),
        toml_edit::Value::Integer(i) => Value::from(*i.value()),
        toml_edit::Value::Float(f) => serde_json::Number::from_f64(*f.value())
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
        other => Value::String(other.to_string().trim().to_string()),
    }
}

pub fn json_to_toml(v: &Value) -> Option<toml_edit::Value> {
    Some(match v {
        Value::String(s) => s.as_str().into(),
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into()
            } else {
                n.as_f64()?.into()
            }
        }
        _ => return None,
    })
}

/// Lectura/escritura de overrides en el documento del usuario.
pub fn user_get(doc: &DocumentMut, section: &str, key: &str) -> Option<Value> {
    doc.get(section)?
        .as_table_like()?
        .get(key)?
        .as_value()
        .map(toml_to_json)
}

pub fn user_set(doc: &mut DocumentMut, section: &str, key: &str, value: &Value) {
    let Some(tv) = json_to_toml(value) else {
        return;
    };
    if !doc.contains_table(section) {
        let mut t = Table::new();
        t.set_implicit(false);
        doc.insert(section, Item::Table(t));
    }
    if let Some(t) = doc[section].as_table_like_mut() {
        // Conserva la decoración (comentarios) si la clave ya existía.
        match t.get_mut(key).and_then(Item::as_value_mut) {
            Some(existing) => {
                let decor = existing.decor().clone();
                *existing = tv;
                *existing.decor_mut() = decor;
            }
            None => {
                t.insert(key, Item::Value(tv));
            }
        }
    }
}

pub fn user_unset(doc: &mut DocumentMut, section: &str, key: &str) {
    let mut now_empty = false;
    if let Some(t) = doc.get_mut(section).and_then(Item::as_table_like_mut) {
        t.remove(key);
        now_empty = t.is_empty();
    }
    if now_empty
        && doc.get(section).and_then(Item::as_table).is_some_and(|t| {
            t.decor()
                .prefix()
                .is_none_or(|p| p.as_str().is_none_or(|s| s.trim().is_empty()))
        })
    {
        doc.remove(section);
    }
}

/// Infiera el tipo de campo a partir del nombre y el valor del tema.
pub fn infer_kind(key: &str, sample: Option<&Value>) -> Kind {
    match sample {
        Some(Value::Bool(_)) => Kind::Bool,
        Some(Value::String(s)) if is_hex_color(s) => Kind::Color,
        Some(Value::String(_)) => Kind::Text,
        Some(Value::Number(n)) if n.is_i64() => {
            if key.ends_with("alpha") {
                float(Some(0.0), Some(1.0), 0.05)
            } else {
                int(Some(0), Some(400), 1)
            }
        }
        Some(Value::Number(_)) => {
            if key.ends_with("alpha") {
                float(Some(0.0), Some(1.0), 0.05)
            } else {
                float(Some(0.0), Some(10.0), 0.05)
            }
        }
        _ => Kind::Text,
    }
}

/// Campos del modo simple: los ajustes que más se tocan.
pub const SIMPLE_KEYS: [(&str, &str); 9] = [
    ("font", "base-size"),
    ("spacing", "scale"),
    ("bar", "size-horizontal"),
    ("bar", "size-vertical"),
    ("bar", "background-alpha"),
    ("popups", "background-alpha"),
    ("menu", "background-alpha"),
    ("launcher", "background-alpha"),
    ("notifications", "background-alpha"),
];

/// Construye la definición de un campo de apariencia.
pub fn field_for(theme: &ThemeToml, section: &str, key: &str) -> FieldDef {
    let tk = theme.get(section, key);
    let sample = tk.and_then(|k| k.value.as_ref().or(k.example.as_ref()));
    let mut kind = infer_kind(key, sample);
    // Rangos razonables para los campos conocidos.
    match (section, key) {
        ("font", "base-size") => kind = int(Some(6), Some(48), 1),
        ("spacing", "scale") => kind = float(Some(0.5), Some(3.0), 0.05),
        ("bar", "size-horizontal") | ("bar", "size-vertical") => kind = int(Some(16), Some(80), 1),
        _ => {}
    }
    let mut f = FieldDef::new(key, key, kind);
    f.default = tk.and_then(|k| k.value.clone());
    f.example = tk.and_then(|k| k.example.clone());
    f.desc = tk.map(|k| k.doc.clone()).unwrap_or_default();
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const THEME: &str = r##"# header
[bar]
# Alpha companions
background       = "#000000"
background-alpha = 1.0
scale-with-font  = true
size-horizontal  = 26

[font]
base-size = 12
# Per-token overrides
# caption       = 10
"##;

    #[test]
    fn parses_theme_with_commented_keys() {
        let t = ThemeToml::parse(THEME);
        assert_eq!(
            t.get("bar", "background").unwrap().value,
            Some(json!("#000000"))
        );
        assert_eq!(t.get("bar", "background").unwrap().doc, "Alpha companions");
        assert_eq!(
            t.get("bar", "background-alpha").unwrap().value,
            Some(json!(1.0))
        );
        let cap = t.get("font", "caption").unwrap();
        assert_eq!(cap.value, None);
        assert_eq!(cap.example, Some(json!(10)));
        assert_eq!(t.sections(), vec!["bar", "font"]);
        assert_eq!(
            infer_kind("background", Some(&json!("#000000"))),
            Kind::Color
        );
    }

    #[test]
    fn user_overrides_preserve_comments() {
        let src = "# mis ajustes\n[font]\n# tamaño\nbase-size = 12\n";
        let mut doc: DocumentMut = src.parse().unwrap();
        user_set(&mut doc, "font", "base-size", &json!(14));
        user_set(&mut doc, "bar", "background-alpha", &json!(0.85));
        let out = doc.to_string();
        assert!(out.contains("# mis ajustes"));
        assert!(out.contains("# tamaño\nbase-size = 14"));
        assert!(out.contains("[bar]\nbackground-alpha = 0.85"));
        assert_eq!(user_get(&doc, "bar", "background-alpha"), Some(json!(0.85)));
        user_unset(&mut doc, "bar", "background-alpha");
        assert!(!doc.to_string().contains("[bar]"));
        user_unset(&mut doc, "font", "base-size");
        assert!(user_get(&doc, "font", "base-size").is_none());
    }
}
