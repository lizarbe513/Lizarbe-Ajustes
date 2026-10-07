//! Búsqueda de opciones: cada panel arma su índice de [`Entry`] y se muestra
//! con el selector grande de siempre (que filtra por varias palabras, sin
//! tildes ni mayúsculas). Al elegir una, `decode` dice a dónde saltar.

use serde_json::{Value, json};

use crate::form::Row;
use crate::popup::{PickItem, PickTarget, Picker, PopupTypes};

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

/// Entrada que lleva a una sección entera (sin descripción, para que una
/// búsqueda de palabras sueltas prefiera la opción a la sección que la contiene).
pub fn section_entry(section: usize, title: &str) -> Entry {
    Entry {
        section,
        section_title: title.to_string(),
        group: String::new(),
        key: String::new(),
        label: title.to_string(),
        desc: String::new(),
    }
}

/// Entradas de las filas de un formulario: cada campo y cada acción, con su
/// encabezado como grupo. La clave es la posición de la fila ([`row_of`]).
pub fn entries_from_rows<B, A>(section: usize, title: &str, rows: &[Row<B, A>]) -> Vec<Entry> {
    let mut out = vec![];
    let mut group = String::new();
    for (i, r) in rows.iter().enumerate() {
        let (label, desc) = match r {
            Row::Header(h) => {
                group = h.clone();
                continue;
            }
            Row::Field(f) => (f.def.label.clone(), f.def.desc.clone()),
            Row::Action(label, _) => (label.clone(), String::new()),
            Row::Note(..) => continue,
        };
        out.push(Entry {
            section,
            section_title: title.to_string(),
            group: group.clone(),
            key: i.to_string(),
            label,
            desc,
        });
    }
    out
}

/// Fila a la que lleva una clave de [`entries_from_rows`].
pub fn row_of(key: &str) -> Option<usize> {
    key.parse().ok()
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

/// Ventana de búsqueda con estas entradas.
pub fn picker<T: PopupTypes>(entries: &[Entry], target: PickTarget<T>) -> Picker<T> {
    Picker {
        title: crate::i18n::t("search.title"),
        items: pick_items(entries),
        sel: 0,
        filter: String::new(),
        target,
        current: None,
        anchor: None,
    }
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
