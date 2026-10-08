//! Textos de Widgets sobre el sistema de idiomas del núcleo.
//!
//! Los diccionarios viven en `src/i18n/{es,en}.json` y se registran en el
//! núcleo la primera vez que se pide un texto.

use std::collections::HashMap;
use std::sync::OnceLock;

use lizarbe_core::i18n as core;
pub use lizarbe_core::i18n::{Lang, lang, set_lang};

const ES: &str = include_str!("i18n/es.json");
const EN: &str = include_str!("i18n/en.json");

fn init() {
    core::init_app(ES, EN);
}

#[cfg(test)]
pub fn t_opt(key: &str) -> Option<String> {
    init();
    core::t_opt(key)
}

pub fn t(key: &str) -> String {
    init();
    core::t(key)
}

pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    init();
    core::tf(key, args)
}

static WIDGETS_ES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Traducción al español de textos que vienen de los manifests de Omarchy
/// (que están en inglés). En inglés se usa el texto original del manifest.
pub fn t_manifest(key: &str) -> Option<String> {
    if lang() != Lang::Es {
        return None;
    }
    WIDGETS_ES
        .get_or_init(|| {
            serde_json::from_str(include_str!("i18n/widgets.es.json"))
                .expect("widgets.es.json válido")
        })
        .get(key)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prefijos de claves que se arman en tiempo de ejecución.
    const DYNAMIC: &[&str] = &[
        "ap.", "bar.", "col.", "cw.", "day.", "idle.", "notif.", "pos.", "restore.",
    ];

    #[test]
    fn dictionaries_are_complete_and_used() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let problems = core::dictionary_problems(ES, EN, &src, DYNAMIC);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
