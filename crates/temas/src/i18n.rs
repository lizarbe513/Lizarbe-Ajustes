//! Textos del estudio de temas sobre el sistema de idiomas del núcleo.

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

    #[test]
    fn dictionaries_have_same_keys() {
        let m = core::key_mismatch(ES, EN);
        assert!(m.is_empty(), "claves distintas: {m:?}");
    }

    #[test]
    fn every_used_key_exists() {
        init();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let missing: Vec<_> = core::used_keys(&src)
            .into_iter()
            .filter(|(_, k)| !k.contains('{') && !core::core_has(k) && core::t_opt(k).is_none())
            .collect();
        assert!(missing.is_empty(), "faltan textos: {missing:?}");
    }
}
