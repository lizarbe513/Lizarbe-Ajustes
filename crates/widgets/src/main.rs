//! Widgets — panel TUI para configurar la barra, los widgets y los plugins
//! de Quickshell en Omarchy.

mod app;
mod i18n;
mod omarchy;
mod store;
mod ui;

use anyhow::Result;
use lizarbe_core::cli::{self, Opt};

use crate::app::{App, Section};
use crate::i18n::t;
use crate::omarchy::paths::Paths;
use crate::store::Store;

const OPTS: &[Opt] = &[
    Opt::value("--section", "<id>", "cli.section"),
    Opt::flag("--advanced", "cli.advanced"),
    Opt::flag("--simple", "cli.simple"),
];

fn main() -> Result<()> {
    let args = cli::parse(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), OPTS, t);
    let section = args.value("--section").map(|v| {
        Section::from_id(v).unwrap_or_else(|| args.fail(&format!("{}: {v}", t("cli.bad_section"))))
    });
    let mut prefs = args.prefs.clone();
    if args.has("--advanced") {
        prefs.advanced = true;
    }
    if args.has("--simple") {
        prefs.advanced = false;
    }
    let store = Store::load(Paths::detect(args.config_dir.clone()));
    let mut app = App::new(store, prefs);
    if let Some(s) = section {
        app.go_section(s);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
