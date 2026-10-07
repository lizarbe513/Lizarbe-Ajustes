//! Textos de la interfaz en español e inglés.
//!
//! Hay dos capas de diccionarios embebidos: los del núcleo (`core/i18n/`,
//! textos comunes) y los de cada aplicación, registrados con [`init_app`].
//! `t("clave")` busca primero en la aplicación y luego en el núcleo, en el
//! idioma activo y después en inglés; si no existe devuelve la propia clave.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

type Dict = HashMap<String, String>;

static CURRENT: AtomicU8 = AtomicU8::new(1);
static CORE: OnceLock<(Dict, Dict)> = OnceLock::new();
static APP: OnceLock<(Dict, Dict)> = OnceLock::new();

fn parse(json: &str, name: &str) -> Dict {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{name}: JSON inválido: {e}"))
}

fn core() -> &'static (Dict, Dict) {
    CORE.get_or_init(|| {
        (
            parse(include_str!("../i18n/es.json"), "core/es.json"),
            parse(include_str!("../i18n/en.json"), "core/en.json"),
        )
    })
}

/// Registra los diccionarios de la aplicación (JSON embebido con
/// `include_str!`). Solo tiene efecto la primera vez.
pub fn init_app(es_json: &'static str, en_json: &'static str) {
    APP.get_or_init(|| (parse(es_json, "es.json"), parse(en_json, "en.json")));
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
    let es = lang() == Lang::Es;
    let pick = |d: &'static (Dict, Dict)| if es { &d.0 } else { &d.1 };
    let app = APP.get();
    let core = core();
    app.and_then(|a| pick(a).get(key))
        .or_else(|| pick(core).get(key))
        .or_else(|| app.and_then(|a| a.1.get(key)))
        .or_else(|| core.1.get(key))
        .cloned()
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

/// Claves del núcleo, para que las aplicaciones validen sus textos.
pub fn core_has(key: &str) -> bool {
    core().0.contains_key(key)
}

/// Claves `t("…")`/`tf("…")` usadas en los `.rs` de `src_dir` (sin contar
/// los propios `i18n.rs`), como `(archivo, clave)`. Sirve a los tests de cada
/// crate para comprobar que no falta ninguna traducción.
pub fn used_keys(src_dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") || p.ends_with("i18n.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            for pat in ["t(\"", "tf(\""] {
                for (i, _) in text.match_indices(pat) {
                    // Evita coincidencias como `fmt(` o `opt(`.
                    let prev = text[..i].chars().last().unwrap_or(' ');
                    if prev.is_alphanumeric() || prev == '_' {
                        continue;
                    }
                    let rest = &text[i + pat.len()..];
                    if let Some(end) = rest.find('"') {
                        out.push((p.clone(), rest[..end].to_string()));
                    }
                }
            }
        }
    }
    out
}

/// Claves presentes en un diccionario y no en el otro.
pub fn key_mismatch(es_json: &str, en_json: &str) -> Vec<String> {
    let es = parse(es_json, "es.json");
    let en = parse(en_json, "en.json");
    let mut missing: Vec<_> = es
        .keys()
        .filter(|k| !en.contains_key(*k))
        .cloned()
        .collect();
    missing.extend(en.keys().filter(|k| !es.contains_key(*k)).cloned());
    missing.sort();
    missing
}

/// Fuente de todos los `.rs` de `dirs` (sin los `i18n.rs`).
fn sources(dirs: &[PathBuf]) -> String {
    let mut out = String::new();
    let mut stack: Vec<PathBuf> = dirs.to_vec();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n.rs") {
                out.push_str(&std::fs::read_to_string(&p).unwrap_or_default());
            }
        }
    }
    out
}

/// Claves de `json` que no aparecen escritas en ningún `.rs` de `dirs` ni
/// empiezan por uno de los prefijos `dynamic` (claves que se arman con
/// `format!`, como `o.{clave}`).
pub fn unused_keys(json: &str, dirs: &[PathBuf], dynamic: &[&str]) -> Vec<String> {
    let src = sources(dirs);
    let mut out: Vec<String> = parse(json, "i18n")
        .into_keys()
        .filter(|k| !dynamic.iter().any(|p| k.starts_with(p)))
        .filter(|k| !src.contains(&format!("\"{k}\"")))
        .collect();
    out.sort();
    out
}

/// Problemas de los diccionarios de una aplicación: claves sin traducir,
/// claves usadas que no existen y claves que sobran. Vacío si todo cuadra.
/// Lo usan los tests de cada crate.
pub fn dictionary_problems(es: &str, en: &str, src: &Path, dynamic: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = key_mismatch(es, en)
        .into_iter()
        .map(|k| format!("sin traducir: {k}"))
        .collect();
    let dict = parse(es, "es.json");
    let mut missing: Vec<String> = used_keys(src)
        .into_iter()
        .filter(|(_, k)| !dict.contains_key(k) && !core_has(k))
        .map(|(p, k)| format!("no existe: {k} ({})", p.display()))
        .collect();
    missing.sort();
    missing.dedup();
    out.extend(missing);
    out.extend(
        unused_keys(es, &[src.to_path_buf()], dynamic)
            .into_iter()
            // Las que repiten una clave del núcleo la sustituyen: las usa el núcleo.
            .filter(|k| !core_has(k))
            .map(|k| format!("sin usar: {k}")),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionaries_have_same_keys() {
        let m = key_mismatch(
            include_str!("../i18n/es.json"),
            include_str!("../i18n/en.json"),
        );
        assert!(m.is_empty(), "claves sin traducir: {m:?}");
    }

    /// Las claves del núcleo las usan el propio núcleo y las aplicaciones.
    #[test]
    fn core_keys_are_used() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let unused = unused_keys(
            include_str!("../i18n/es.json"),
            &[crates],
            &["key.", "val.", "mb."],
        );
        assert!(unused.is_empty(), "claves del núcleo sin usar: {unused:?}");
    }

    #[test]
    fn parses_locale() {
        assert_eq!(Lang::parse("es_PE.UTF-8"), Some(Lang::Es));
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::parse("fr_FR"), None);
    }

    #[test]
    fn all_used_keys_exist() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing: Vec<String> = used_keys(&src)
            .into_iter()
            .filter(|(_, k)| !core_has(k))
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
}
