//! Bucle principal de una TUI: dibujo, teclado, ratón, tareas periódicas y
//! ejecución de comandos externos con la interfaz suspendida.

use std::io::{self, BufRead, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// Lo que recibe `after_command` como error cuando el usuario canceló el comando
/// (Ctrl+C, o Esc si la aplicación lo pidió con `esc_cancels`).
pub const CANCELLED: &str = "cancelled";

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
    /// Si Esc cancela los comandos externos (como Ctrl+C), p. ej. al pedir la
    /// contraseña de sudo. Una flecha o Alt+tecla también envían Esc.
    fn esc_cancels(&self) -> bool {
        false
    }
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
                let res = run_external(&cmd, title, app.esc_cancels());
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

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Espera a que termine el comando. Si el usuario lo interrumpió y sigue vivo
/// poco después (un script de bash que continúa con el siguiente `sudo`), lo termina.
fn esperar(proc: &mut std::process::Command) -> io::Result<std::process::ExitStatus> {
    let mut hijo = proc.spawn()?;
    let mut desde: Option<Instant> = None;
    let mut terminado = false;
    loop {
        if let Some(st) = hijo.try_wait()? {
            return Ok(st);
        }
        if INTERRUPTED.load(Ordering::SeqCst) {
            let t0 = *desde.get_or_insert_with(Instant::now);
            if !terminado && t0.elapsed() > Duration::from_millis(150) {
                // SAFETY: se envía una señal a un proceso hijo propio, todavía sin recoger.
                unsafe { libc::kill(hijo.id() as libc::pid_t, libc::SIGTERM) };
                terminado = true;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `stty` sobre la terminal de la que se lee; `None` si no hay terminal o falla.
fn stty(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("stty")
        .args(args)
        .stdin(Stdio::inherit())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Hace que Esc interrumpa como Ctrl+C. Devuelve el estado anterior para restaurarlo.
fn esc_as_interrupt() -> Option<String> {
    let saved = stty(&["-g"])?;
    stty(&["intr", "^["])?;
    Some(saved)
}

/// Ejecuta un comando con la terminal normal y espera Enter para volver. Si el
/// usuario lo interrumpe (Ctrl+C, o Esc con `esc_cancels`) devuelve
/// `Err(CANCELLED)` y vuelve enseguida.
fn run_external(cmd: &Command, app_name: &str, esc_cancels: bool) -> Result<bool, String> {
    println!("\x1b[1m$ {} {}\x1b[0m", cmd.program, cmd.args.join(" "));
    if esc_cancels {
        println!(
            "\x1b[2m{}\x1b[0m",
            tf("exec.esc_hint", &[("app", app_name)])
        );
    }
    println!();
    let saved = if esc_cancels {
        esc_as_interrupt()
    } else {
        None
    };
    let mut proc = std::process::Command::new(&cmd.program);
    proc.args(&cmd.args);
    // SAFETY: solo se restablece la acción de SIGINT, que es segura entre fork y exec.
    unsafe {
        proc.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
    // La interrupción llega a todo el grupo en primer plano, también a la
    // aplicación: se anota (en vez de cerrarse) porque comandos como sudo la
    // atienden y terminan con un código normal, sin morir por la señal.
    INTERRUPTED.store(false, Ordering::SeqCst);
    // SAFETY: el manejador solo escribe en un atómico, que es seguro en una señal.
    let previous =
        unsafe { libc::signal(libc::SIGINT, on_sigint as *const () as libc::sighandler_t) };
    let status = esperar(&mut proc);
    unsafe { libc::signal(libc::SIGINT, previous) };
    if let Some(s) = saved {
        stty(&[&s]);
    }
    let interrupted = INTERRUPTED.swap(false, Ordering::SeqCst)
        || status
            .as_ref()
            .is_ok_and(|s| s.signal() == Some(libc::SIGINT));
    if interrupted {
        return Err(CANCELLED.to_string());
    }
    let res = match status {
        Ok(s) => Ok(s.success()),
        Err(e) => Err(format!("{}: {e}", cmd.program)),
    };
    print!("\n{}", tf("exec.press_enter", &[("app", app_name)]));
    let _ = io::stdout().flush();
    let _ = io::stdin().lock().read_line(&mut String::new());
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_command_is_reported_as_cancelled() {
        let cmd = Command {
            program: "sh".into(),
            args: vec!["-c".into(), "kill -INT $$".into()],
        };
        assert_eq!(run_external(&cmd, "t", false), Err(CANCELLED.to_string()));
        assert_eq!(run_external(&cmd, "t", true), Err(CANCELLED.to_string()));
        // Como sudo: el comando atiende la interrupción y sale con código normal.
        let cmd = Command {
            program: "sh".into(),
            args: vec!["-c".into(), "kill -INT $PPID; exit 1".into()],
        };
        assert_eq!(run_external(&cmd, "t", false), Err(CANCELLED.to_string()));
        // Un script que sigue tras la interrupción (como el de Omarchy con su segundo sudo)
        // se termina en vez de dejarlo pedir otra contraseña.
        let cmd = Command {
            program: "sh".into(),
            args: vec!["-c".into(), "kill -INT $PPID; sleep 30; exit 0".into()],
        };
        let t0 = Instant::now();
        assert_eq!(run_external(&cmd, "t", false), Err(CANCELLED.to_string()));
        assert!(t0.elapsed() < Duration::from_secs(5), "no esperó al sleep");
    }
}
