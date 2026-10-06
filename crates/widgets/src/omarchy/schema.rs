//! Campos configurables: la definición genérica vive en el núcleo; aquí se
//! construyen desde el `schema` de los manifests de Omarchy.

pub use lizarbe_core::schema::*;
use serde_json::Value;

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
}
