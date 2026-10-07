//! Textos del estudio de temas sobre el sistema de idiomas del núcleo.

use lizarbe_core::i18n as core;
pub use lizarbe_core::i18n::lang;
#[cfg(test)]
pub use lizarbe_core::i18n::{Lang, set_lang};

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

    /// Prefijos de claves que se arman en tiempo de ejecución.
    const DYNAMIC: &[&str] = &["c.", "tab."];

    #[test]
    fn dictionaries_are_complete_and_used() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let problems = core::dictionary_problems(ES, EN, &src, DYNAMIC);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
