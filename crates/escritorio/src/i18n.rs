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
    /// en los dos idiomas (incluida una pantalla de ejemplo).
    #[test]
    fn catalog_is_translated() {
        let mut m = lizarbe_core::hypr::Monitor::named("HDMI-A-1");
        m.width = 1920;
        m.height = 1080;
        m.refresh_rate = 60.0;
        m.available_modes = vec!["1920x1080@60.00Hz".into()];
        let ctx = crate::catalog::Ctx {
            monitors: vec![m],
            cursor_themes: vec!["Adwaita".into()],
        };
        for lang in [Lang::Es, Lang::En] {
            set_lang(lang);
            for section in crate::catalog::Section::ALL {
                assert!(!section.title().starts_with("sec."), "{section:?}");
                for g in crate::catalog::groups(section, &ctx) {
                    assert!(!g.title.starts_with("g."), "grupo {}", g.id);
                    assert!(!g.title.contains("mon."), "grupo {}", g.id);
                    for f in g.fields {
                        for text in [&f.label, &f.desc] {
                            assert!(
                                !text.starts_with("o.") && !text.starts_with("mon."),
                                "{}: falta texto",
                                f.key
                            );
                        }
                        let opts = match &f.kind {
                            lizarbe_core::schema::Kind::Enum(o) => o.clone(),
                            _ => f.presets.clone(),
                        };
                        for o in opts {
                            assert!(
                                !o.label.starts_with("o.") && !o.label.starts_with("mon."),
                                "{}: opción sin texto {}",
                                f.key,
                                o.label
                            );
                        }
                    }
                }
            }
        }
    }
}
