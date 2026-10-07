//! Textos de la tienda sobre el sistema de idiomas del núcleo.

use lizarbe_core::i18n as core;

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
    fn dictionaries_are_complete_and_used() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let problems = core::dictionary_problems(ES, EN, &src, &[]);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
