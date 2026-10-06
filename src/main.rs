//! meca-qs — panel TUI para configurar los widgets Quickshell de Omarchy.

mod app;
mod i18n;
mod omarchy;
mod prefs;
mod store;
mod ui;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

use crate::app::{App, ExecRequest};
use crate::i18n::{Lang, t};
use crate::omarchy::paths::Paths;
use crate::prefs::Prefs;
use crate::store::Store;

struct Args {
    lang: Option<Lang>,
    config_dir: Option<PathBuf>,
    advanced: Option<bool>,
}

fn usage() -> String {
    format!(
        "meca-qs {}\n\n{}\n\n  --lang <es|en>        {}\n  --config-dir <dir>    {}\n  --advanced            {}\n  --simple              {}\n  -h, --help\n  -V, --version\n",
        env!("CARGO_PKG_VERSION"),
        t("cli.about"),
        t("cli.lang"),
        t("cli.config_dir"),
        t("cli.advanced"),
        t("cli.simple"),
    )
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lang: None,
        config_dir: None,
        advanced: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lang" => {
                let v = it.next().ok_or("--lang <es|en>")?;
                args.lang = Some(Lang::parse(&v).ok_or("--lang <es|en>")?);
            }
            "--config-dir" => {
                args.config_dir = Some(PathBuf::from(it.next().ok_or("--config-dir <dir>")?));
            }
            "--advanced" => args.advanced = Some(true),
            "--simple" => args.advanced = Some(false),
            "-h" | "--help" => {
                print!("{}", usage());
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("meca-qs {}", env!("CARGO_PKG_VERSION"));
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
            eprintln!("meca-qs: {e}");
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
        eprintln!("meca-qs: {}: {}", t("cli.no_dir"), dir.display());
        std::process::exit(2);
    }

    let store = Store::load(Paths::detect(args.config_dir));
    let mut app = App::new(store, prefs);
    run(&mut app)
}

fn enter() -> ratatui::DefaultTerminal {
    let terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture, SetTitle("Quickshell"));
    terminal
}

fn leave() {
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
}

fn run(app: &mut App) -> Result<()> {
    let mut terminal = enter();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|f| ui::draw(f, app))?;
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.on_key(k),
                    Event::Mouse(m) => app.on_mouse(m),
                    _ => {}
                }
            }
            app.tick();
            if let Some(req) = app.exec.take() {
                leave();
                let res = run_external(&req);
                terminal = enter();
                terminal.clear()?;
                app.after_exec(&req, res);
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    leave();
    result
}

/// Ejecuta un comando con la terminal normal y espera Enter para volver.
fn run_external(req: &ExecRequest) -> Result<bool, String> {
    println!("\x1b[1m$ {} {}\x1b[0m\n", req.program, req.args.join(" "));
    let status = Command::new(&req.program).args(&req.args).status();
    let res = match status {
        Ok(s) => Ok(s.success()),
        Err(e) => Err(format!("{}: {e}", req.program)),
    };
    print!("\n{}", t("exec.press_enter"));
    let _ = io::stdout().flush();
    let _ = io::stdin().lock().read_line(&mut String::new());
    res
}
