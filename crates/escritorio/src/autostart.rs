//! Inicio automático: las líneas `o.launch_on_start("…")` y
//! `o.exec_on_start("…")` de `~/.config/hypr/autostart.lua`. Desactivar un
//! programa comenta su línea; el resto del archivo no se toca.

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub cmd: String,
    /// `exec_on_start` (orden suelta) en vez de `launch_on_start` (programa).
    pub exec: bool,
    pub enabled: bool,
    /// Línea del archivo de donde se leyó; `None` si es nueva.
    pub line: Option<usize>,
}

impl Entry {
    pub fn new(cmd: &str, exec: bool) -> Entry {
        Entry {
            cmd: cmd.to_string(),
            exec,
            enabled: true,
            line: None,
        }
    }

    fn to_line(&self) -> String {
        let f = if self.exec {
            "exec_on_start"
        } else {
            "launch_on_start"
        };
        let call = format!("o.{f}({:?})", self.cmd);
        if self.enabled {
            call
        } else {
            format!("-- {call}")
        }
    }
}

pub fn parse(text: &str) -> Vec<Entry> {
    let mut out = vec![];
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        let (enabled, l) = match l.strip_prefix("--") {
            Some(rest) => (false, rest.trim()),
            None => (true, l),
        };
        for (f, exec) in [("o.launch_on_start(", false), ("o.exec_on_start(", true)] {
            if let Some(arg) = l.strip_prefix(f).and_then(|r| r.strip_suffix(')'))
                && let Some(cmd) = arg
                    .trim()
                    .strip_prefix('"')
                    .and_then(|a| a.strip_suffix('"'))
                && !cmd.contains('"')
            {
                out.push(Entry {
                    cmd: cmd.to_string(),
                    exec,
                    enabled,
                    line: Some(i),
                });
            }
        }
    }
    out
}

/// El archivo con las líneas de `entries` cambiadas, las quitadas borradas y
/// las nuevas al final.
pub fn render(text: &str, entries: &[Entry]) -> String {
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        match parse(line).first() {
            Some(_) => {
                if let Some(e) = entries.iter().find(|e| e.line == Some(i)) {
                    out.push_str(&e.to_line());
                    out.push('\n');
                }
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    for e in entries.iter().filter(|e| e.line.is_none()) {
        out.push_str(&e.to_line());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "-- Extra autostart processes.\n-- o.launch_on_start(\"hyprsunset\")\no.exec_on_start(\"notify-send hola\")\n-- texto suelto\n";

    #[test]
    fn reads_enabled_and_commented() {
        let e = parse(FILE);
        assert_eq!(e.len(), 2);
        assert_eq!(
            e[0],
            Entry {
                cmd: "hyprsunset".into(),
                exec: false,
                enabled: false,
                line: Some(1)
            }
        );
        assert!(e[1].exec && e[1].enabled);
    }

    #[test]
    fn toggles_removes_and_appends() {
        let mut e = parse(FILE);
        e[0].enabled = true;
        e.remove(1);
        e.push(Entry::new("blueman-applet", false));
        let out = render(FILE, &e);
        assert_eq!(
            out,
            "-- Extra autostart processes.\no.launch_on_start(\"hyprsunset\")\n-- texto suelto\no.launch_on_start(\"blueman-applet\")\n"
        );
        assert_eq!(render(FILE, &parse(FILE)), FILE, "sin cambios queda igual");
    }
}
