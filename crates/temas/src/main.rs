//! Estudio de temas — crea y edita temas completos de Omarchy (colores,
//! bordes, fondos, iconos, Neovim, VS Code, barra y menús) con una maqueta
//! en vivo y la opción de probarlos en el escritorio real.

mod app;
mod draft;
mod i18n;
mod images;
mod mockup;
mod palette;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use lizarbe_core::prefs::Prefs;
use lizarbe_core::theme::Palette;
use lizarbe_core::themes::Dirs;

use crate::app::App;
use crate::i18n::{Lang, t};

const BIN: &str = env!("CARGO_PKG_NAME");

struct Args {
    lang: Option<Lang>,
    config_dir: Option<PathBuf>,
    theme: Option<String>,
    new_from: Option<String>,
}

fn usage() -> String {
    format!(
        "{BIN} {}\n\n{}\n\n  --theme <nombre>      {}\n  --new <base>          {}\n  --lang <es|en>        {}\n  --config-dir <dir>    {}\n  -h, --help\n  -V, --version\n",
        env!("CARGO_PKG_VERSION"),
        t("cli.about"),
        t("cli.theme"),
        t("cli.new"),
        t("cli.lang"),
        t("cli.config_dir"),
    )
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lang: None,
        config_dir: None,
        theme: None,
        new_from: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => {
                let v = it.next().ok_or("--lang <es|en>")?;
                args.lang = Some(Lang::parse(&v).ok_or("--lang <es|en>")?);
            }
            "--theme" => args.theme = Some(it.next().ok_or("--theme <nombre>")?),
            "--new" => args.new_from = Some(it.next().ok_or("--new <base>")?),
            "--config-dir" => {
                args.config_dir = Some(PathBuf::from(it.next().ok_or("--config-dir <dir>")?));
            }
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
    let prefs = Prefs::load();
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
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/state"));
    let omarchy = std::env::var_os("OMARCHY_PATH")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("/usr/share/omarchy"));
    let sandbox = args.config_dir.is_some();
    if let Some(dir) = &args.config_dir
        && !dir.is_dir()
    {
        eprintln!("{BIN}: {}: {}", t("cli.no_dir"), dir.display());
        std::process::exit(2);
    }
    let user = match &args.config_dir {
        Some(d) => d.join("themes"),
        None => home.join(".config/omarchy/themes"),
    };
    let _ = std::fs::create_dir_all(&user);
    let dirs = Dirs {
        user,
        system: omarchy.join("themes"),
    };
    let colors = state.join("omarchy/current/theme/colors.toml");
    let pal = Palette::load(&colors);
    let cache = dirs::cache_dir()
        .unwrap_or_else(|| home.join(".cache"))
        .join("lizarbe/temas");
    // Pregunta a la terminal qué gráficos admite (sixel, kitty…); si no responde, semibloques.
    let picker = if sandbox {
        ratatui_image::picker::Picker::halfblocks()
    } else {
        ratatui_image::picker::Picker::from_query_stdio()
            .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks())
    };
    let mut app = App::new(dirs, pal, sandbox, cache, picker, colors);
    if let Some(slug) = args.theme {
        app.open_theme(&slug);
    } else if let Some(base) = args.new_from {
        app.start_new(&base);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
