//! Búsqueda de opciones: cada panel arma su índice de [`Entry`] y se muestra
//! con el selector grande de siempre (que filtra por varias palabras, sin
//! tildes ni mayúsculas). Al elegir una, `decode` dice a dónde saltar.

use serde_json::{Value, json};

use crate::popup::PickItem;

/// Una opción que se puede encontrar.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Índice de la sección o pestaña donde vive.
    pub section: usize,
    pub section_title: String,
    pub group: String,
    /// Identificador de la opción (su clave) dentro de la sección.
    pub key: String,
    pub label: String,
    pub desc: String,
}

/// Las entradas como elementos del selector, agrupadas por sección y grupo.
pub fn pick_items(entries: &[Entry]) -> Vec<PickItem> {
    entries
        .iter()
        .map(|e| PickItem {
            label: e.label.clone(),
            detail: e.desc.clone(),
            group: if e.group.is_empty() {
                e.section_title.clone()
            } else {
                format!("{} › {}", e.section_title, e.group)
            },
            value: json!({ "s": e.section, "k": e.key }),
            enabled: true,
        })
        .collect()
}

/// Sección y clave de la opción elegida.
pub fn decode(v: &Value) -> Option<(usize, String)> {
    Some((
        v.get("s")?.as_u64()? as usize,
        v.get("k")?.as_str()?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popup::{PickTarget, Picker, PopupTypes};

    #[derive(Debug, Clone, PartialEq)]
    struct X;
    impl PopupTypes for X {
        type Bind = String;
        type Confirm = ();
        type Input = ();
        type Pick = ();
        type Menu = ();
    }

    fn entry(section: usize, group: &str, key: &str, label: &str, desc: &str) -> Entry {
        Entry {
            section,
            section_title: format!("Sección {section}"),
            group: group.into(),
            key: key.into(),
            label: label.into(),
            desc: desc.into(),
        }
    }

    fn picker(items: Vec<PickItem>, filter: &str) -> Picker<X> {
        Picker {
            title: String::new(),
            items,
            sel: 0,
            filter: filter.into(),
            target: PickTarget::App(()),
            current: None,
            anchor: None,
        }
    }

    #[test]
    fn finds_by_words_without_accents_in_any_order() {
        let es = vec![
            entry(
                0,
                "Bordes",
                "b:size",
                "Grosor del borde",
                "Ancho de la línea",
            ),
            entry(1, "Cursor", "c:size", "Tamaño del cursor", "En píxeles"),
        ];
        let items = pick_items(&es);
        assert_eq!(picker(items.clone(), "tamano cursor").visible(), vec![1]);
        assert_eq!(picker(items.clone(), "BORDE grosor").visible(), vec![0]);
        assert_eq!(picker(items.clone(), "pixeles").visible(), vec![1]);
        assert_eq!(picker(items.clone(), "").visible(), vec![0, 1]);
        assert!(picker(items, "xyz").visible().is_empty());
    }

    #[test]
    fn value_round_trips() {
        let items = pick_items(&[entry(3, "G", "k:1", "Etiqueta", "")]);
        assert_eq!(decode(&items[0].value), Some((3, "k:1".to_string())));
        assert_eq!(items[0].group, "Sección 3 › G");
    }
}
