//! Arranque común de las aplicaciones: idioma (preferencias, `--lang`),
//! `--config-dir` (modo de pruebas), `--help` y `--version`. Cada aplicación
//! declara sus opciones propias y las lee de [`Args`].

use std::path::PathBuf;

use crate::i18n::{self, Lang};
use crate::prefs::Prefs;

/// Una opción propia de la aplicación.
pub struct Opt {
    /// `--nombre`.
    pub flag: &'static str,
    /// Si lleva valor, cómo se muestra en la ayuda (`<id>`).
    pub value: Option<&'static str>,
    /// Clave del texto de ayuda.
    pub help: &'static str,
}

impl Opt {
    pub const fn flag(flag: &'static str, help: &'static str) -> Opt {
        Opt {
            flag,
            value: None,
            help,
        }
    }

    pub const fn value(flag: &'static str, value: &'static str, help: &'static str) -> Opt {
        Opt {
            flag,
            value: Some(value),
            help,
        }
    }
}

/// Lo que se leyó de la línea de órdenes.
pub struct Args {
    pub prefs: Prefs,
    pub config_dir: Option<PathBuf>,
    given: Vec<(&'static str, Option<String>)>,
    usage: String,
    bin: &'static str,
}

impl Args {
    /// ¿Se pasó la opción `flag`?
    pub fn has(&self, flag: &str) -> bool {
        self.given.iter().any(|(f, _)| *f == flag)
    }

    /// Valor de la opción `flag` (la última vez que se pasó).
    pub fn value(&self, flag: &str) -> Option<&str> {
        self.given
            .iter()
            .rev()
            .find(|(f, _)| *f == flag)
            .and_then(|(_, v)| v.as_deref())
    }

    /// Termina con un error de uso (código 2) y la ayuda.
    pub fn fail(&self, msg: &str) -> ! {
        eprintln!("{}: {msg}\n\n{}", self.bin, self.usage);
        std::process::exit(2)
    }
}

fn usage(bin: &str, version: &str, about: &str, opts: &[Opt], t: fn(&str) -> String) -> String {
    let mut lines: Vec<(String, String)> = opts
        .iter()
        .map(|o| {
            let name = match o.value {
                Some(v) => format!("{} {v}", o.flag),
                None => o.flag.to_string(),
            };
            (name, t(o.help))
        })
        .collect();
    lines.push(("--lang <es|en>".into(), t("cli.lang")));
    lines.push(("--config-dir <dir>".into(), t("cli.config_dir")));
    let w = lines.iter().map(|(n, _)| n.len()).max().unwrap_or(0) + 4;
    let mut out = format!("{bin} {version}\n\n{about}\n\n");
    for (n, h) in lines {
        out.push_str(&format!("  {n:<w$}{h}\n"));
    }
    out.push_str("  -h, --help\n  -V, --version\n");
    out
}

/// Carga las preferencias, fija el idioma y lee los argumentos. Sale del
/// programa con `--help`, `--version` o un argumento que no entiende.
pub fn parse(bin: &'static str, version: &str, opts: &[Opt], t: fn(&str) -> String) -> Args {
    let prefs = Prefs::load();
    i18n::set_lang(Lang::parse(&prefs.lang).unwrap_or_else(Lang::from_env));
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // `--lang` primero, para que la ayuda y los errores salgan en ese idioma.
    if let Some(l) = raw
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| raw.get(i + 1))
        .and_then(|v| Lang::parse(v))
    {
        i18n::set_lang(l);
    }
    let help = usage(bin, version, &t("cli.about"), opts, t);
    let mut args = Args {
        prefs,
        config_dir: None,
        given: vec![],
        usage: help,
        bin,
    };
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{}", args.usage);
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{bin} {version}");
                std::process::exit(0);
            }
            "--lang" => {
                if it.next().and_then(|v| Lang::parse(&v)).is_none() {
                    args.fail("--lang <es|en>");
                }
            }
            "--config-dir" => match it.next() {
                Some(d) => args.config_dir = Some(PathBuf::from(d)),
                None => args.fail("--config-dir <dir>"),
            },
            other => {
                let Some(o) = opts.iter().find(|o| o.flag == other) else {
                    args.fail(other)
                };
                let v = match o.value {
                    Some(placeholder) => match it.next() {
                        Some(v) => Some(v),
                        None => args.fail(&format!("{} {placeholder}", o.flag)),
                    },
                    None => None,
                };
                args.given.push((o.flag, v));
            }
        }
    }
    if let Some(dir) = &args.config_dir
        && !dir.is_dir()
    {
        args.fail(&format!("{}: {}", t("cli.no_dir"), dir.display()));
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(k: &str) -> String {
        format!("<{k}>")
    }

    #[test]
    fn usage_lists_own_and_common_options() {
        let opts = [
            Opt::value("--section", "<id>", "cli.section"),
            Opt::flag("--advanced", "cli.advanced"),
        ];
        let u = usage("app", "1.0", "Acerca", &opts, tr);
        assert!(u.starts_with("app 1.0\n\nAcerca\n"));
        for needle in [
            "--section <id>",
            "<cli.section>",
            "--advanced",
            "--lang <es|en>",
            "--config-dir <dir>",
            "-V, --version",
        ] {
            assert!(u.contains(needle), "{needle}\n{u}");
        }
    }
}
