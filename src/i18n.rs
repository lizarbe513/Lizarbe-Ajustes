//! Textos de la interfaz en español e inglés.
//!
//! Los diccionarios viven en `src/i18n/{es,en}.json` y se embeben en el binario.
//! `t("clave")` devuelve el texto del idioma activo, con el inglés como respaldo
//! y la propia clave si no existe en ninguno.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Es => "es",
            Lang::En => "en",
        }
    }

    pub fn parse(s: &str) -> Option<Lang> {
        let s = s.trim().to_lowercase();
        if s.starts_with("es") {
            Some(Lang::Es)
        } else if s.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }

    /// Idioma según el entorno ($LC_ALL, $LC_MESSAGES, $LANG); inglés por defecto.
    pub fn from_env() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var)
                && !v.is_empty()
                && v != "C"
                && v != "POSIX"
            {
                return Lang::parse(&v).unwrap_or(Lang::En);
            }
        }
        Lang::En
    }

    pub fn toggle(self) -> Lang {
        match self {
            Lang::Es => Lang::En,
            Lang::En => Lang::Es,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(1);
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
static DICTS: OnceLock<(HashMap<String, String>, HashMap<String, String>)> = OnceLock::new();

fn dicts() -> &'static (HashMap<String, String>, HashMap<String, String>) {
    DICTS.get_or_init(|| {
        let es = serde_json::from_str(include_str!("i18n/es.json")).expect("es.json válido");
        let en = serde_json::from_str(include_str!("i18n/en.json")).expect("en.json válido");
        (es, en)
    })
}

pub fn set_lang(lang: Lang) {
    CURRENT.store(
        match lang {
            Lang::Es => 0,
            Lang::En => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn lang() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 0 {
        Lang::Es
    } else {
        Lang::En
    }
}

/// Texto si existe la clave (útil para traducciones opcionales).
pub fn t_opt(key: &str) -> Option<String> {
    let (es, en) = dicts();
    let primary = if lang() == Lang::Es { es } else { en };
    primary.get(key).or_else(|| en.get(key)).cloned()
}

pub fn t(key: &str) -> String {
    t_opt(key).unwrap_or_else(|| key.to_string())
}

/// Texto con sustituciones `{nombre}`.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionaries_have_same_keys() {
        let (es, en) = dicts();
        let mut missing: Vec<_> = es.keys().filter(|k| !en.contains_key(*k)).collect();
        missing.extend(en.keys().filter(|k| !es.contains_key(*k)));
        missing.sort();
        assert!(missing.is_empty(), "claves sin traducir: {missing:?}");
    }

    #[test]
    fn parses_locale() {
        assert_eq!(Lang::parse("es_PE.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::parse("fr_FR"), None);
    }

    /// Todas las claves `t("…")`/`tf("…")` usadas en el código existen.
    #[test]
    fn all_used_keys_exist() {
        let (es, _) = dicts();
        let mut missing = vec![];
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut stack = vec![std::path::PathBuf::from(src)];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || p.ends_with("i18n.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                for pat in ["t(\"", "tf(\""] {
                    for (i, _) in text.match_indices(pat) {
                        // Evita coincidencias como `fmt(` o `opt(`.
                        let prev = text[..i].chars().last().unwrap_or(' ');
                        if prev.is_alphanumeric() || prev == '_' {
                            continue;
                        }
                        let rest = &text[i + pat.len()..];
                        if let Some(end) = rest.find('"') {
                            let key = &rest[..end];
                            if !es.contains_key(key) {
                                missing.push(format!("{}: {key}", p.display()));
                            }
                        }
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "claves inexistentes:\n{}",
            missing.join("\n")
        );
    }
}
