//! Tienda de aplicaciones de Lizarbe: busca e instala programas por
//! categorías, sin necesidad de conocer el nombre del paquete.

mod app;
mod catalogo;
mod i18n;
mod sistema;
mod ui;

use anyhow::Result;
use lizarbe_core::cli::{self, Opt};
use lizarbe_core::paths as p;
use lizarbe_core::theme::Palette;

use crate::app::App;
use crate::catalogo::{Catalogo, Fuente};
use crate::i18n::t;
use crate::sistema::Estado;

const OPTS: &[Opt] = &[
    Opt::value("--fuente", "<repos|aur>", "cli.fuente"),
    Opt::value("--categoria", "<id>", "cli.cat"),
    Opt::value("--app", "<id>", "cli.app"),
    Opt::value("--buscar", "<texto>", "cli.search"),
];

fn main() -> Result<()> {
    let args = cli::parse(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), OPTS, t);
    let pal = Palette::load(&p::current_theme().join("colors.toml"));
    let fuente = match args.value("--fuente") {
        None => Fuente::Repos,
        Some(f) => Fuente::parse(f).unwrap_or_else(|| args.fail(&t("cli.bad_fuente"))),
    };
    let mut app = App::new(Catalogo::embebido(), Estado::cargar(), pal, fuente);
    if let Some(c) = args.value("--categoria") {
        app.ir_a_categoria(c);
    }
    if let Some(a) = args.value("--app") {
        app.ir_a_app(a);
    }
    if let Some(q) = args.value("--buscar") {
        app.buscar(q);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
