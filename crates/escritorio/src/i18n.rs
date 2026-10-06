//! Textos de Escritorio sobre el sistema de idiomas del núcleo.

use lizarbe_core::i18n as core;
pub use lizarbe_core::i18n::{Lang, lang, set_lang};

const ES: &str = include_str!("i18n/es.json");
const EN: &str = include_str!("i18n/en.json");

fn init() {
    core::init_app(ES, EN);
}

pub fn t(key: &str) -> String {
    init();
    core::t(key)
}

pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    init();
    core::tf(key, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn dictionaries_have_same_keys() {
        let m = core::key_mismatch(ES, EN);
        assert!(m.is_empty(), "claves sin traducir: {m:?}");
    }

    #[test]
    fn all_used_keys_exist() {
        let es: HashMap<String, String> = serde_json::from_str(ES).unwrap();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing: Vec<String> = core::used_keys(&src)
            .into_iter()
            .filter(|(_, k)| !es.contains_key(k) && !core::core_has(k))
            .map(|(p, k)| format!("{}: {k}", p.display()))
            .collect();
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "claves inexistentes:\n{}",
            missing.join("\n")
        );
    }

    /// Cada ajuste del catálogo tiene nombre, descripción y textos de opciones
    /// en los dos idiomas.
    #[test]
    fn catalog_is_translated() {
        let es: HashMap<String, String> = serde_json::from_str(ES).unwrap();
        let en: HashMap<String, String> = serde_json::from_str(EN).unwrap();
        for section in crate::catalog::Section::ALL {
            for g in crate::catalog::groups(section) {
                assert!(es.contains_key(&format!("g.{}", g.id)), "grupo {}", g.id);
                for f in g.fields {
                    for k in [format!("o.{}", f.key), format!("o.{}.d", f.key)] {
                        assert!(es.contains_key(&k) && en.contains_key(&k), "falta {k}");
                    }
                    if let lizarbe_core::schema::Kind::Enum(opts) = &f.kind {
                        for o in opts {
                            assert!(!o.label.starts_with("o."), "opción sin texto: {}", o.label);
                        }
                    }
                }
            }
        }
    }
}
