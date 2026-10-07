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

use anyhow::Result;
use lizarbe_core::cli::{self, Opt};
use lizarbe_core::paths as p;
use lizarbe_core::theme::Palette;
use lizarbe_core::themes::Dirs;

use crate::app::App;
use crate::i18n::t;

const OPTS: &[Opt] = &[
    Opt::value("--theme", "<nombre>", "cli.theme"),
    Opt::value("--new", "<base>", "cli.new"),
];

fn main() -> Result<()> {
    let args = cli::parse(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), OPTS, t);
    let sandbox = args.config_dir.is_some();
    let user = match &args.config_dir {
        Some(d) => d.join("themes"),
        None => p::home().join(".config/omarchy/themes"),
    };
    let _ = std::fs::create_dir_all(&user);
    let dirs = Dirs {
        user,
        system: p::omarchy().join("themes"),
    };
    let colors = p::current_theme().join("colors.toml");
    let pal = Palette::load(&colors);
    let cache = dirs::cache_dir()
        .unwrap_or_else(|| p::home().join(".cache"))
        .join("lizarbe/temas");
    // Pregunta a la terminal qué gráficos admite (sixel, kitty…); si no responde, semibloques.
    let picker = if sandbox {
        ratatui_image::picker::Picker::halfblocks()
    } else {
        ratatui_image::picker::Picker::from_query_stdio()
            .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks())
    };
    let mut app = App::new(dirs, pal, sandbox, cache, picker, colors);
    if let Some(slug) = args.value("--theme") {
        app.open_theme(slug);
    } else if let Some(base) = args.value("--new") {
        app.start_new(base);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
