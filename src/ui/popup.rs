//! Ventanas emergentes: confirmación, mensajes, entrada de texto, selector
//! con filtro y lista de casillas.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::omarchy::schema::{FieldDef, Opt};
use crate::store::Bind;

/// Qué hacer cuando el usuario confirma.
#[derive(Debug, Clone, PartialEq)]
pub enum Confirm {
    Apply,
    Discard,
    Quit,
    RestartShell,
    TogglePlugin(String),
    Restore(crate::store::Scope),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputTarget {
    Field { bind: Bind, def: Box<FieldDef> },
    NewRawKey { entry: Bind },
    RawValue { entry: Bind, key: String },
    GitUrl,
    NewModule { qml: bool, at: (usize, usize) },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PickTarget {
    Field { bind: Bind, def: Box<FieldDef> },
    AddWidget { at: (usize, usize) },
}

#[derive(Debug, Clone)]
pub struct PickItem {
    pub label: String,
    pub detail: String,
    pub group: String,
    pub value: Value,
    pub enabled: bool,
}

impl PickItem {
    pub fn from_opt(o: &Opt) -> Self {
        PickItem {
            label: o.label.clone(),
            detail: o.desc.clone(),
            group: String::new(),
            value: o.value.clone(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Input {
    pub title: String,
    pub hint: String,
    pub buf: Vec<char>,
    pub cursor: usize,
    pub error: Option<String>,
    pub target: InputTarget,
}

impl Input {
    pub fn new(title: String, hint: String, text: &str, target: InputTarget) -> Self {
        let buf: Vec<char> = text.chars().collect();
        Input {
            title,
            hint,
            cursor: buf.len(),
            buf,
            error: None,
            target,
        }
    }

    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }
}

#[derive(Debug, Clone)]
pub struct Picker {
    pub title: String,
    pub items: Vec<PickItem>,
    pub sel: usize,
    pub filter: String,
    pub target: PickTarget,
    pub current: Option<Value>,
    /// Si existe, se dibuja como desplegable pegado a ese control (estilo
    /// Meca); si no, como ventana centrada con buscador.
    pub anchor: Option<ratatui::layout::Rect>,
}

/// Minúsculas y sin acentos, para buscar "micro" y encontrar "Micrófono".
pub fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

impl Picker {
    /// Índices visibles según el filtro.
    pub fn visible(&self) -> Vec<usize> {
        let f = fold(&self.filter);
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                f.is_empty()
                    || fold(&it.label).contains(&f)
                    || fold(&it.detail).contains(&f)
                    || fold(&it.group).contains(&f)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Checklist {
    pub title: String,
    pub opts: Vec<Opt>,
    pub checked: Vec<bool>,
    pub sel: usize,
    pub bind: Bind,
}

/// Acción de una entrada del menú contextual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAct {
    Activate(usize),
    ResetField(usize),
    WidgetSettings(usize, usize),
    WidgetMove(usize, usize, usize),
    WidgetAdd(usize),
    WidgetRemove(usize, usize),
    /// Tecla a reenviar a la lista de plugins (`E` = Enter).
    PluginKey(usize, char),
    Apply,
    Cancel,
    Restore,
    Help,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub icon: &'static str,
    pub label: String,
    pub act: MenuAct,
    pub enabled: bool,
}

/// Menú contextual (clic derecho o tecla `o`).
#[derive(Debug, Clone)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub sel: usize,
    /// Esquina donde se abre (posición del ratón).
    pub at: (u16, u16),
}

#[derive(Debug, Clone)]
pub enum Popup {
    Confirm {
        title: String,
        lines: Vec<String>,
        action: Confirm,
    },
    Message {
        title: String,
        lines: Vec<String>,
    },
    /// Archivos cambiados por fuera mientras había cambios pendientes.
    Conflict {
        files: Vec<&'static str>,
    },
    Input(Input),
    Picker(Picker),
    Checklist(Checklist),
    Help {
        scroll: u16,
    },
    Menu(Menu),
}

impl Popup {
    /// Botones clicables de la ventana: (clave de texto, tecla equivalente).
    pub fn buttons(&self) -> Vec<(&'static str, KeyCode)> {
        match self {
            Popup::Confirm { .. } => vec![("mb.cancel", KeyCode::Esc), ("mb.ok", KeyCode::Enter)],
            Popup::Message { .. } => vec![("mb.ok", KeyCode::Enter)],
            Popup::Help { .. } => vec![("mb.close", KeyCode::Enter)],
            Popup::Conflict { .. } => vec![
                ("mb.cancel", KeyCode::Esc),
                ("mb.reload", KeyCode::Char('r')),
                ("mb.overwrite", KeyCode::Char('o')),
            ],
            Popup::Input(_) | Popup::Checklist(_) => {
                vec![("mb.cancel", KeyCode::Esc), ("mb.save", KeyCode::Enter)]
            }
            Popup::Picker(p) if p.anchor.is_none() => {
                vec![("mb.cancel", KeyCode::Esc), ("mb.choose", KeyCode::Enter)]
            }
            Popup::Picker(_) | Popup::Menu(_) => vec![],
        }
    }
}

/// Resultado de procesar una tecla dentro de un popup.
#[derive(Debug)]
pub enum Outcome {
    /// Sigue abierto.
    Stay,
    Close,
    Confirmed(Confirm),
    Submitted(InputTarget, String),
    Picked(PickTarget, Value),
    Checked(Bind, Vec<Value>),
    ConflictOverwrite,
    ConflictReload,
    MenuPick(MenuAct),
}

fn edit_line(input: &mut Input, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('u') if ctrl => {
            input.buf.clear();
            input.cursor = 0;
        }
        KeyCode::Char('a') if ctrl => input.cursor = 0,
        KeyCode::Char('e') if ctrl => input.cursor = input.buf.len(),
        KeyCode::Char(c) if !ctrl => {
            input.buf.insert(input.cursor, c);
            input.cursor += 1;
        }
        KeyCode::Backspace => {
            if input.cursor > 0 {
                input.cursor -= 1;
                input.buf.remove(input.cursor);
            }
        }
        KeyCode::Delete => {
            if input.cursor < input.buf.len() {
                input.buf.remove(input.cursor);
            }
        }
        KeyCode::Left => input.cursor = input.cursor.saturating_sub(1),
        KeyCode::Right => input.cursor = (input.cursor + 1).min(input.buf.len()),
        KeyCode::Home => input.cursor = 0,
        KeyCode::End => input.cursor = input.buf.len(),
        _ => return false,
    }
    input.error = None;
    true
}

impl Popup {
    pub fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match self {
            Popup::Confirm { action, .. } => match key.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('s') => {
                    Outcome::Confirmed(action.clone())
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => Outcome::Close,
                _ => Outcome::Stay,
            },
            Popup::Help { scroll } => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    *scroll = scroll.saturating_sub(1);
                    Outcome::Stay
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *scroll = scroll.saturating_add(1);
                    Outcome::Stay
                }
                KeyCode::PageDown | KeyCode::Char(' ') => {
                    *scroll = scroll.saturating_add(10);
                    Outcome::Stay
                }
                KeyCode::PageUp => {
                    *scroll = scroll.saturating_sub(10);
                    Outcome::Stay
                }
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                    Outcome::Close
                }
                _ => Outcome::Stay,
            },
            Popup::Message { .. } => match key.code {
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                    Outcome::Close
                }
                _ => Outcome::Stay,
            },
            Popup::Conflict { .. } => match key.code {
                KeyCode::Char('o') => Outcome::ConflictOverwrite,
                KeyCode::Char('r') => Outcome::ConflictReload,
                KeyCode::Esc | KeyCode::Char('q') => Outcome::Close,
                _ => Outcome::Stay,
            },
            Popup::Input(input) => match key.code {
                KeyCode::Esc => Outcome::Close,
                KeyCode::Enter => Outcome::Submitted(input.target.clone(), input.text()),
                _ => {
                    edit_line(input, key);
                    Outcome::Stay
                }
            },
            Popup::Picker(p) => {
                let vis = p.visible();
                let pos = vis.iter().position(|&i| i == p.sel);
                match key.code {
                    KeyCode::Esc => {
                        if p.filter.is_empty() {
                            return Outcome::Close;
                        }
                        p.filter.clear();
                    }
                    KeyCode::Enter => {
                        if let Some(it) = p.items.get(p.sel)
                            && pos.is_some()
                            && it.enabled
                        {
                            return Outcome::Picked(p.target.clone(), it.value.clone());
                        }
                    }
                    KeyCode::Up => {
                        if let Some(i) = pos.and_then(|i| i.checked_sub(1)) {
                            p.sel = vis[i];
                        }
                    }
                    KeyCode::Down => {
                        if let Some(&n) = pos.map(|i| i + 1).and_then(|i| vis.get(i)) {
                            p.sel = n;
                        }
                    }
                    KeyCode::PageUp | KeyCode::Home => {
                        if let Some(&f) = vis.first() {
                            p.sel = f;
                        }
                    }
                    KeyCode::PageDown | KeyCode::End => {
                        if let Some(&l) = vis.last() {
                            p.sel = l;
                        }
                    }
                    KeyCode::Backspace => {
                        p.filter.pop();
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        p.filter.push(c);
                    }
                    _ => {}
                }
                // Mantén la selección dentro de lo visible.
                let vis = p.visible();
                if !vis.contains(&p.sel)
                    && let Some(&f) = vis.first()
                {
                    p.sel = f;
                }
                Outcome::Stay
            }
            Popup::Menu(m) => {
                let n = m.items.len();
                let step = |from: usize, fwd: bool| {
                    let mut i = from;
                    for _ in 0..n {
                        i = if fwd { (i + 1) % n } else { (i + n - 1) % n };
                        if m.items[i].enabled {
                            return i;
                        }
                    }
                    from
                };
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('o') => Outcome::Close,
                    KeyCode::Up | KeyCode::Char('k') => {
                        m.sel = step(m.sel, false);
                        Outcome::Stay
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        m.sel = step(m.sel, true);
                        Outcome::Stay
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => match m.items.get(m.sel) {
                        Some(it) if it.enabled => Outcome::MenuPick(it.act),
                        _ => Outcome::Stay,
                    },
                    _ => Outcome::Stay,
                }
            }
            Popup::Checklist(c) => match key.code {
                KeyCode::Esc => Outcome::Close,
                KeyCode::Up | KeyCode::Char('k') => {
                    c.sel = c.sel.saturating_sub(1);
                    Outcome::Stay
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    c.sel = (c.sel + 1).min(c.opts.len().saturating_sub(1));
                    Outcome::Stay
                }
                KeyCode::Char(' ') => {
                    if let Some(x) = c.checked.get_mut(c.sel) {
                        *x = !*x;
                    }
                    Outcome::Stay
                }
                KeyCode::Char('a') => {
                    let all = c.checked.iter().all(|x| *x);
                    c.checked.iter_mut().for_each(|x| *x = !all);
                    Outcome::Stay
                }
                KeyCode::Enter => Outcome::Checked(
                    c.bind.clone(),
                    c.opts
                        .iter()
                        .zip(&c.checked)
                        .filter(|(_, on)| **on)
                        .map(|(o, _)| o.value.clone())
                        .collect(),
                ),
                _ => Outcome::Stay,
            },
        }
    }
}
