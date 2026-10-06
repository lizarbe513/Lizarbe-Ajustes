//! Modelo de `~/.config/omarchy/shell.json`.
//!
//! Se trabaja sobre `serde_json::Value` (con orden preservado) para no perder
//! nunca claves que esta herramienta no conoce.

use serde_json::{Map, Value, json};

pub const SECTIONS: [&str; 3] = ["left", "center", "right"];

/// Segmento de una ruta dentro del JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Idx(usize),
}

pub fn key(k: &str) -> Seg {
    Seg::Key(k.to_string())
}

/// Ruta a la entrada `index` de la sección `section` de la barra.
pub fn entry_path(section: usize, index: usize) -> Vec<Seg> {
    vec![
        key("bar"),
        key("layout"),
        key(SECTIONS[section]),
        Seg::Idx(index),
    ]
}

pub fn path_to_string(path: &[Seg]) -> String {
    let mut out = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(k);
            }
            Seg::Idx(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

/// Garantiza la forma mínima que el shell espera (igual que `omarchy-shell-config`).
pub fn normalize(v: &mut Value) {
    if !v.is_object() {
        *v = json!({});
    }
    let obj = v.as_object_mut().unwrap();
    if !obj.get("version").is_some_and(|x| x == 1) {
        obj.insert("version".into(), json!(1));
    }
    let bar = obj.entry("bar").or_insert_with(|| json!({}));
    if !bar.is_object() {
        *bar = json!({});
    }
    let layout = bar
        .as_object_mut()
        .unwrap()
        .entry("layout")
        .or_insert_with(|| json!({}));
    if !layout.is_object() {
        *layout = json!({});
    }
    for s in SECTIONS {
        let sec = layout
            .as_object_mut()
            .unwrap()
            .entry(s)
            .or_insert_with(|| json!([]));
        if !sec.is_array() {
            *sec = json!([]);
        }
    }
    let plugins = obj.entry("plugins").or_insert_with(|| json!([]));
    if !plugins.is_array() {
        *plugins = json!([]);
    }
}

pub fn get<'a>(v: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => cur.as_object()?.get(k)?,
            Seg::Idx(i) => cur.as_array()?.get(*i)?,
        };
    }
    Some(cur)
}

pub fn get_mut<'a>(v: &'a mut Value, path: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = v;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => cur.as_object_mut()?.get_mut(k)?,
            Seg::Idx(i) => cur.as_array_mut()?.get_mut(*i)?,
        };
    }
    Some(cur)
}

/// Escribe `value` en `path`, creando objetos intermedios. Una entrada de la
/// barra escrita como cadena (`"omarchy.clock"`) se convierte en `{"id": ...}`.
pub fn set(v: &mut Value, path: &[Seg], value: Value) -> bool {
    let Some((last, parents)) = path.split_last() else {
        *v = value;
        return true;
    };
    let mut cur = v;
    for seg in parents {
        if let Value::String(s) = cur {
            *cur = json!({ "id": s.clone() });
        }
        cur = match seg {
            Seg::Key(k) => {
                if !cur.is_object() {
                    *cur = json!({});
                }
                cur.as_object_mut()
                    .unwrap()
                    .entry(k.clone())
                    .or_insert_with(|| json!({}))
            }
            Seg::Idx(i) => match cur.as_array_mut().and_then(|a| a.get_mut(*i)) {
                Some(x) => x,
                None => return false,
            },
        };
    }
    if let Value::String(s) = cur {
        *cur = json!({ "id": s.clone() });
    }
    match last {
        Seg::Key(k) => {
            if !cur.is_object() {
                *cur = json!({});
            }
            cur.as_object_mut().unwrap().insert(k.clone(), value);
            true
        }
        Seg::Idx(i) => match cur.as_array_mut().and_then(|a| a.get_mut(*i)) {
            Some(x) => {
                *x = value;
                true
            }
            None => false,
        },
    }
}

/// Elimina la clave final de `path` (no hace nada si no existe).
pub fn remove(v: &mut Value, path: &[Seg]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let Some(parent) = get_mut(v, parents) else {
        return;
    };
    match last {
        Seg::Key(k) => {
            if let Some(obj) = parent.as_object_mut() {
                obj.shift_remove(k);
            }
        }
        Seg::Idx(i) => {
            if let Some(arr) = parent.as_array_mut()
                && *i < arr.len()
            {
                arr.remove(*i);
            }
        }
    }
}

/// Id de una entrada de la barra (objeto con `id` o cadena simple).
pub fn entry_id(entry: &Value) -> String {
    match entry {
        Value::String(s) => s.clone(),
        Value::Object(o) => o
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

pub fn section(v: &Value, section: usize) -> &[Value] {
    get(v, &[key("bar"), key("layout"), key(SECTIONS[section])])
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn section_mut(v: &mut Value, section: usize) -> &mut Vec<Value> {
    normalize(v);
    get_mut(v, &[key("bar"), key("layout"), key(SECTIONS[section])])
        .and_then(Value::as_array_mut)
        .expect("normalize garantiza la sección")
}

pub fn insert_entry(v: &mut Value, section: usize, index: usize, entry: Value) {
    let sec = section_mut(v, section);
    let index = index.min(sec.len());
    sec.insert(index, entry);
}

pub fn remove_entry(v: &mut Value, section: usize, index: usize) -> Option<Value> {
    let sec = section_mut(v, section);
    (index < sec.len()).then(|| sec.remove(index))
}

/// Mueve una entrada; `to_index` se interpreta sobre la sección destino ya
/// sin la entrada movida. Devuelve la posición final.
pub fn move_entry(
    v: &mut Value,
    from: (usize, usize),
    to_section: usize,
    to_index: usize,
) -> Option<(usize, usize)> {
    let entry = remove_entry(v, from.0, from.1)?;
    let sec = section_mut(v, to_section);
    let idx = to_index.min(sec.len());
    sec.insert(idx, entry);
    Some((to_section, idx))
}

/// Dónde aparece un id en la barra: (sección, índice).
pub fn find_in_bar(v: &Value, id: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for s in 0..3 {
        for (i, e) in section(v, s).iter().enumerate() {
            if entry_id(e) == id {
                out.push((s, i));
            }
        }
    }
    out
}

pub fn plugins_list(v: &Value) -> &[Value] {
    get(v, &[key("plugins")])
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn disabled_list(v: &Value) -> Vec<String> {
    get(v, &[key("disabledPlugins")])
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_disabled(v: &mut Value, id: &str, disabled: bool) {
    let mut list = disabled_list(v);
    list.retain(|x| x != id);
    if disabled {
        list.push(id.to_string());
    }
    let obj = v.as_object_mut().expect("normalizado");
    if list.is_empty() {
        obj.shift_remove("disabledPlugins");
    } else {
        obj.insert(
            "disabledPlugins".into(),
            Value::Array(list.into_iter().map(Value::String).collect()),
        );
    }
}

pub fn bar_option(v: &Value) -> String {
    get(v, &[key("bar"), key("id")])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("omarchy.bar")
        .to_string()
}

pub fn in_plugins(v: &Value, id: &str) -> bool {
    plugins_list(v).iter().any(|e| entry_id(e) == id)
}

pub fn set_in_plugins(v: &mut Value, id: &str, present: bool) {
    normalize(v);
    let arr = get_mut(v, &[key("plugins")])
        .and_then(Value::as_array_mut)
        .unwrap();
    let exists = arr.iter().any(|e| entry_id(e) == id);
    if present && !exists {
        arr.push(json!({ "id": id }));
    } else if !present {
        arr.retain(|e| entry_id(e) != id);
    }
}

/// Claves de una entrada de la barra, excepto `id`.
pub fn entry_settings(entry: &Value) -> Map<String, Value> {
    match entry {
        Value::Object(o) => o
            .iter()
            .filter(|(k, _)| k.as_str() != "id")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        _ => Map::new(),
    }
}

/// Serializa como lo dejaría un editor humano: 2 espacios y salto final.
pub fn to_pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::from_str(
            r#"{
              "zeta": {"keep": true},
              "bar": {"position": "top", "layout": {
                "left": [{"id": "omarchy.menu"}, "omarchy.workspaces"],
                "center": [{"id": "omarchy.clock", "format": "HH:mm"}],
                "right": []
              }},
              "idle": {"lock": 300},
              "plugins": [],
              "version": 1
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_preserves_unknown_keys_and_order() {
        let v = sample();
        let text = to_pretty(&v);
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v, back);
        let keys: Vec<_> = back.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["zeta", "bar", "idle", "plugins", "version"]);
    }

    #[test]
    fn string_entries_become_objects_when_edited() {
        let mut v = sample();
        let mut p = entry_path(0, 1);
        p.push(key("foo"));
        assert!(set(&mut v, &p, json!(1)));
        assert_eq!(
            section(&v, 0)[1],
            json!({"id": "omarchy.workspaces", "foo": 1})
        );
        assert_eq!(entry_id(&section(&v, 0)[0]), "omarchy.menu");
    }

    #[test]
    fn move_between_sections() {
        let mut v = sample();
        let to = move_entry(&mut v, (0, 0), 2, 0).unwrap();
        assert_eq!(to, (2, 0));
        assert_eq!(entry_id(&section(&v, 2)[0]), "omarchy.menu");
        assert_eq!(section(&v, 0).len(), 1);
        // Mover dentro de la misma sección.
        insert_entry(&mut v, 2, 9, json!({"id": "x"}));
        let to = move_entry(&mut v, (2, 1), 2, 0).unwrap();
        assert_eq!(to, (2, 0));
        assert_eq!(entry_id(&section(&v, 2)[0]), "x");
    }

    #[test]
    fn remove_and_set_paths() {
        let mut v = sample();
        let mut p = entry_path(1, 0);
        p.push(key("format"));
        remove(&mut v, &p);
        assert_eq!(section(&v, 1)[0], json!({"id": "omarchy.clock"}));
        set(&mut v, &[key("idle"), key("screensaver")], json!(60));
        assert_eq!(
            get(&v, &[key("idle"), key("screensaver")]),
            Some(&json!(60))
        );
    }

    #[test]
    fn disabled_plugins_list_is_dropped_when_empty() {
        let mut v = sample();
        set_disabled(&mut v, "omarchy.osd", true);
        assert_eq!(disabled_list(&v), vec!["omarchy.osd"]);
        set_disabled(&mut v, "omarchy.osd", false);
        assert!(v.get("disabledPlugins").is_none());
    }

    #[test]
    fn normalize_fixes_shape() {
        let mut v = json!({"bar": {"layout": {"left": 3}}});
        normalize(&mut v);
        assert_eq!(v["version"], json!(1));
        assert!(v["bar"]["layout"]["left"].is_array());
        assert!(v["bar"]["layout"]["right"].is_array());
        assert!(v["plugins"].is_array());
    }
}
