//! Escritorio — panel TUI para configurar la apariencia y el comportamiento
//! de las ventanas de Hyprland en Omarchy, sin editar archivos.

mod app;
mod apps;
mod autostart;
mod binds;
mod capture;
mod catalog;
mod hyprfile;
mod i18n;
mod migrate;
mod paths;
mod record;
mod store;
mod sunset;
mod system;
mod ui;
mod xcompose;

use anyhow::Result;
use lizarbe_core::cli::{self, Opt};

use crate::app::{App, Section};
use crate::i18n::t;
use crate::paths::Paths;
use crate::store::Store;

const OPTS: &[Opt] = &[
    Opt::value("--section", "<id>", "cli.section"),
    Opt::flag("--migrate-meca", "cli.migrate"),
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

    let mut store = Store::load(Paths::detect(args.config_dir.clone()));
    if args.has("--migrate-meca") {
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
    if let Some(s) = section {
        app.go_section(s);
    }
    lizarbe_core::term::run(&mut app, &t("app.name"))
}
