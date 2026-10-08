//! Eventos de Hyprland en vivo (`.socket2.sock`): cambio de escritorio,
//! ventanas que se abren o cierran, capas (menús)… Se leen en un hilo y se
//! entregan por un canal, que la aplicación vacía en su `tick`.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

/// Un evento: `workspace>>2` → `name = "workspace"`, `data = "2"`.
#[derive(Debug, Clone, PartialEq)]
pub struct HyprEvent {
    pub name: String,
    pub data: String,
}

/// Interpreta una línea del socket (`nombre>>datos`).
pub fn parse_line(line: &str) -> Option<HyprEvent> {
    let (name, data) = line.trim_end().split_once(">>")?;
    (!name.is_empty()).then(|| HyprEvent {
        name: name.to_string(),
        data: data.to_string(),
    })
}

/// Ruta del socket de eventos de la sesión actual.
pub fn socket_path() -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(
        PathBuf::from(runtime)
            .join("hypr")
            .join(sig)
            .join(".socket2.sock"),
    )
}

/// Escucha en segundo plano. Al soltarla, el hilo termina con el siguiente evento.
pub struct Listener {
    rx: Receiver<HyprEvent>,
}

impl Listener {
    /// `None` si no hay sesión de Hyprland a la que conectar.
    pub fn start() -> Option<Listener> {
        let stream = UnixStream::connect(socket_path()?).ok()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                if let Some(ev) = parse_line(&line)
                    && tx.send(ev).is_err()
                {
                    break;
                }
            }
        });
        Some(Listener { rx })
    }

    /// Eventos llegados desde la última vez.
    pub fn drain(&self) -> Vec<HyprEvent> {
        self.rx.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_lines() {
        let e = parse_line("workspace>>2\n").unwrap();
        assert_eq!((e.name.as_str(), e.data.as_str()), ("workspace", "2"));
        // Los datos pueden traer más `>>` o ir vacíos.
        let e = parse_line("openwindow>>80a,2,kitty,~ >> x").unwrap();
        assert_eq!(e.name, "openwindow");
        assert_eq!(e.data, "80a,2,kitty,~ >> x");
        assert_eq!(parse_line("submap>>").unwrap().data, "");
        assert!(parse_line("sin separador").is_none());
        assert!(parse_line(">>x").is_none());
    }

    #[test]
    fn reads_events_from_a_socket() {
        use std::io::Write;
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("lz-ev-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let server = UnixListener::bind(&path).unwrap();
        let stream = UnixStream::connect(&path).unwrap();
        let (mut peer, _) = server.accept().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                if let Some(ev) = parse_line(&line.unwrap()) {
                    tx.send(ev).unwrap();
                }
            }
        });
        let l = Listener { rx };
        peer.write_all(b"workspace>>3\nactivewindow>>kitty,~\n")
            .unwrap();
        let mut got = vec![];
        for _ in 0..100 {
            got.extend(l.drain());
            if got.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].data, "3");
        std::fs::remove_dir_all(&dir).ok();
    }
}
