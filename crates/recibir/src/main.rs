//! Recibir archivos del teléfono con KDE Connect: explica cómo enviarlos
//! desde la app del teléfono y avisa de lo que va llegando.

mod app;
mod dibujo;
mod i18n;
mod ui;

use anyhow::Result;
use lizarbe_core::cli;

use crate::app::App;
use crate::i18n::t;

fn main() -> Result<()> {
    let args = cli::parse(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), &[], t);
    let mut app = App::new(args.config_dir.is_some());
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
