//! Bucle principal de una TUI: dibujo, teclado, ratón, tareas periódicas y
//! ejecución de comandos externos con la interfaz suspendida.

use std::io::{self, BufRead, Write};
use std::time::Duration;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind, MouseEvent,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::SetTitle;

use crate::i18n::tf;

/// Comando externo que se ejecuta en la terminal normal.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

pub trait TuiApp {
    fn draw(&mut self, f: &mut Frame);
    fn on_key(&mut self, key: KeyEvent);
    fn on_mouse(&mut self, m: MouseEvent);
    /// Se llama varias veces por segundo (avisos que caducan, recargas).
    fn tick(&mut self);
    /// Comando pendiente de ejecutar fuera de la interfaz.
    fn take_command(&mut self) -> Option<Command>;
    /// Resultado del último comando: `Ok(éxito)` o el error al lanzarlo.
    fn after_command(&mut self, result: Result<bool, String>);
    fn should_quit(&self) -> bool;
}

fn enter(title: &str) -> ratatui::DefaultTerminal {
    let terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture, SetTitle(title));
    terminal
}

fn leave() {
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
}

/// Ejecuta la aplicación hasta que pida salir. `title` es el título de la
/// ventana de la terminal.
pub fn run<A: TuiApp>(app: &mut A, title: &str) -> Result<()> {
    let mut terminal = enter(title);
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|f| app.draw(f))?;
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.on_key(k),
                    Event::Mouse(m) => app.on_mouse(m),
                    _ => {}
                }
            }
            app.tick();
            if let Some(cmd) = app.take_command() {
                leave();
                let res = run_external(&cmd, title);
                terminal = enter(title);
                terminal.clear()?;
                app.after_command(res);
            }
            if app.should_quit() {
                return Ok(());
            }
        }
    })();
    leave();
    result
}

/// Ejecuta un comando con la terminal normal y espera Enter para volver.
fn run_external(cmd: &Command, app_name: &str) -> Result<bool, String> {
    println!("\x1b[1m$ {} {}\x1b[0m\n", cmd.program, cmd.args.join(" "));
    let status = std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .status();
    let res = match status {
        Ok(s) => Ok(s.success()),
        Err(e) => Err(format!("{}: {e}", cmd.program)),
    };
    print!("\n{}", tf("exec.press_enter", &[("app", app_name)]));
    let _ = io::stdout().flush();
    let _ = io::stdin().lock().read_line(&mut String::new());
    res
}
