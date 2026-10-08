//! Bienvenida de Lizarbe: guía de primer inicio. Enseña los atajos con
//! práctica en vivo (Hyprland cuenta lo que el usuario hace), conecta el
//! teléfono, deja probar la terminal y recorre las herramientas del sistema.

mod anim;
mod app;
mod brand;
mod caps;
mod contenido;
mod i18n;
mod qr;
mod retos;
mod sistema;
mod ui;

use anyhow::Result;
use lizarbe_core::cli::{self, Opt};

use crate::app::App;
use crate::i18n::t;

const OPTS: &[Opt] = &[
    Opt::value("--capitulo", "<0-9>", "cli.capitulo"),
    Opt::flag("--demo", "cli.demo"),
];

fn main() -> Result<()> {
    let args = cli::parse(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), OPTS, t);
    let capitulo = match args.value("--capitulo") {
        None => 0,
        Some(v) => v
            .parse::<usize>()
            .ok()
            .filter(|n| *n < app::Cap::ALL.len())
            .unwrap_or_else(|| args.fail(&t("cli.bad_capitulo"))),
    };
    // Con --config-dir tampoco se toca nada del sistema.
    let demo = args.has("--demo") || args.config_dir.is_some();
    let mut app = App::new(demo, capitulo);
    lizarbe_core::term::run(&mut app, &t("app.name"))?;
    // Al salir: o se deja la marca para volver a mostrarla, o se quita.
    if !demo && !app.volver_a_mostrar {
        let _ = std::fs::remove_file(sistema::marca_pendiente());
    }
    Ok(())
}
