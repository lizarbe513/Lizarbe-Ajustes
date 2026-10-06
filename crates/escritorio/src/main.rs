//! Escritorio — panel TUI para configurar la apariencia y el comportamiento
//! de las ventanas de Hyprland en Omarchy, sin editar archivos.

mod app;
mod apps;
mod autostart;
mod binds;
mod catalog;
mod hyprfile;
mod i18n;
mod migrate;
mod paths;
mod record;
mod store;
mod sunset;
mod themes;
mod ui;
mod xcompose;

use std::path::PathBuf;

use anyhow::Result;
use lizarbe_core::prefs::Prefs;

use crate::app::{App, Section};
use crate::i18n::{Lang, t};
use crate::paths::Paths;
use crate::store::Store;

const BIN: &str = env!("CARGO_PKG_NAME");

struct Args {
    lang: Option<Lang>,
    config_dir: Option<PathBuf>,
    advanced: Option<bool>,
    section: Option<Section>,
    migrate: bool,
}

fn usage() -> String {
    format!(
        "{BIN} {}\n\n{}\n\n  --section <id>        {}\n  --lang <es|en>        {}\n  --config-dir <dir>    {}\n  --migrate-meca        {}\n  --advanced            {}\n  --simple              {}\n  -h, --help\n  -V, --version\n",
        env!("CARGO_PKG_VERSION"),
        t("cli.about"),
        t("cli.section"),
        t("cli.lang"),
        t("cli.config_dir"),
        t("cli.migrate"),
        t("cli.advanced"),
        t("cli.simple"),
    )
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lang: None,
        config_dir: None,
        advanced: None,
        section: None,
        migrate: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => {
                let v = it.next().ok_or("--lang <es|en>")?;
                args.lang = Some(Lang::parse(&v).ok_or("--lang <es|en>")?);
            }
            "--section" => {
                let v = it.next().ok_or("--section <id>")?;
                args.section = Some(
                    Section::from_id(&v)
                        .ok_or_else(|| format!("{}: {v}\n\n{}", t("cli.bad_section"), usage()))?,
                );
            }
            "--config-dir" => {
                args.config_dir = Some(PathBuf::from(it.next().ok_or("--config-dir <dir>")?));
            }
            "--migrate-meca" => args.migrate = true,
            "--advanced" => args.advanced = Some(true),
            "--simple" => args.advanced = Some(false),
            "-h" | "--help" => {
                print!("{}", usage());
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{BIN} {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => return Err(format!("{other}\n\n{}", usage())),
        }
    }
    Ok(args)
}

fn main() -> Result<()> {
    let mut prefs = Prefs::load();
    i18n::set_lang(Lang::parse(&prefs.lang).unwrap_or_else(Lang::from_env));
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{BIN}: {e}");
            std::process::exit(2);
        }
    };
    if let Some(l) = args.lang {
        i18n::set_lang(l);
    }
    if let Some(adv) = args.advanced {
        prefs.advanced = adv;
    }
    if let Some(dir) = &args.config_dir
        && !dir.is_dir()
    {
        eprintln!("{BIN}: {}: {}", t("cli.no_dir"), dir.display());
        std::process::exit(2);
    }

    let mut store = Store::load(Paths::detect(args.config_dir));
    if args.migrate {
        if !store.meca_pending() {
            println!("{}", t("mig.nothing"));
            return Ok(());
        }
        let r = store.migrate_meca()?;
        println!(
            "{}",
            i18n::tf(
                "mig.done",
                &[("n", &r.settings.to_string()), ("b", &r.binds.to_string())]
            )
        );
        return Ok(());
    }
    let mut app = App::new(store, prefs);
    app.offer_migration();
    if let Some(s) = args.section {
        app.go_section(s);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
