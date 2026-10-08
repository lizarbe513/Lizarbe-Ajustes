//! Textos de la Bienvenida sobre el sistema de idiomas del núcleo.

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

    /// Prefijos de claves que se arman al dibujar: `reto.<id>`, `con.<id>`, `herr.<id>`, `pie.<capítulo>`.
    const DYNAMIC: &[&str] = &[
        "reto.", "con.", "herr.", "pie.", "cap.", "tel.paso", "tel.est.", "update.",
    ];

    /// Claves que se arman al dibujar: existen en los dos idiomas.
    #[test]
    fn dynamic_keys_exist_in_both_languages() {
        use crate::contenido::{CONCEPTOS, HERRAMIENTAS};
        let mut claves: Vec<String> = vec![];
        for r in crate::retos::Reto::TODOS {
            for sufijo in ["", ".pista"] {
                claves.push(format!("reto.{}{sufijo}", r.id()));
            }
        }
        for c in CONCEPTOS {
            for sufijo in ["", ".1", ".2"] {
                claves.push(format!("con.{}{sufijo}", c.id));
            }
        }
        for h in HERRAMIENTAS {
            for sufijo in ["", ".d", ".ruta"] {
                claves.push(format!("herr.{}{sufijo}", h.id));
            }
        }
        for c in [
            "super",
            "practica",
            "terminal",
            "telefono",
            "escritorio",
            "configurar",
            "conceptos",
            "atajos",
        ] {
            claves.push(format!("cap.{c}"));
        }
        for n in 1..=3 {
            claves.push(format!("tel.paso{n}"));
        }
        for e in ["conectado", "lejos", "solicitado", "disponible"] {
            claves.push(format!("tel.est.{e}"));
        }
        for n in 1..=3 {
            claves.push(format!("update.f{n}"));
        }
        let es: std::collections::HashMap<String, String> = serde_json_like(ES);
        let en: std::collections::HashMap<String, String> = serde_json_like(EN);
        let faltan: Vec<_> = claves
            .iter()
            .filter(|k| !es.contains_key(*k) || !en.contains_key(*k))
            .collect();
        assert!(faltan.is_empty(), "faltan: {faltan:?}");
    }

    /// JSON plano `{"clave": "texto"}` sin dependencias extra.
    fn serde_json_like(json: &str) -> std::collections::HashMap<String, String> {
        let mut out = std::collections::HashMap::new();
        for linea in json.lines() {
            let l = linea.trim();
            if let Some(rest) = l.strip_prefix('"')
                && let Some((k, v)) = rest.split_once("\": \"")
            {
                out.insert(
                    k.to_string(),
                    v.trim_end_matches(',').trim_end_matches('"').to_string(),
                );
            }
        }
        out
    }

    #[test]
    fn dictionaries_are_complete_and_used() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let problems = core::dictionary_problems(ES, EN, &src, DYNAMIC);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
