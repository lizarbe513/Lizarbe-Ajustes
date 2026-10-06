//! Dibujo de la interfaz, con el estilo visual de Meca: barra de título en
//! color de acento, sidebar por categorías, filas de tres líneas con controles
//! en relieve, botones 3D y barra de estado inferior. Todo elemento clicable
//! reacciona al pasar el ratón.

pub mod form;
mod modal;
pub mod popup;
mod views;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus, Hit, Section};
use crate::i18n::{self, t, tf};
use form::NoteKind;
use popup::Popup;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, header);
    let side_w = if area.width < 95 { 22 } else { 26 };
    let [side, sep, content] = Layout::horizontal([
        Constraint::Length(side_w),
        Constraint::Length(1),
        Constraint::Min(20),
    ])
    .areas(body);
    draw_sidebar(f, app, side);
    for y in sep.y..sep.bottom() {
        put(f, sep.x, y, vec![Span::styled("│", fg(app.pal.muted))]);
    }
    views::draw_content(f, app, content);
    draw_footer(f, app, footer);

    if app.popup.is_some() {
        modal::draw(f, app, area);
    }
}

// ---------------------------------------------------------------- utilidades

pub(crate) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

/// ¿Está el ratón sobre este elemento (y no hay ventana encima)?
pub(crate) fn hovered(app: &App, hit: Hit) -> bool {
    app.popup.is_none() && app.hover == Some(hit)
}

/// Escribe una línea de `spans` en (x, y), recortada al ancho disponible.
pub(crate) fn put(f: &mut Frame, x: u16, y: u16, spans: Vec<Span>) -> u16 {
    let area = f.area();
    if y >= area.bottom() || x >= area.right() {
        return 0;
    }
    let w: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
    let w = w.min(area.right() - x);
    f.render_widget(Line::from(spans), Rect::new(x, y, w, 1));
    w
}

pub(crate) fn pad(s: &str, w: usize) -> String {
    let cur = s.width();
    if cur >= w {
        truncate(s, w)
    } else {
        format!("{s}{}", " ".repeat(w - cur))
    }
}

pub(crate) fn truncate(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

/// Ajuste de línea sencillo por palabras.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut out = vec![];
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.width() + 1 + word.width() > width {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while line.width() > width {
                let cut: String = line.chars().take(width).collect();
                line = line.chars().skip(width).collect();
                out.push(cut);
            }
        }
        out.push(line);
    }
    out
}

pub(crate) fn center(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return truncate(s, w);
    }
    let left = (w - cw) / 2;
    format!("{}{s}{}", " ".repeat(left), " ".repeat(w - cw - left))
}

pub(crate) fn right_align(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return truncate(s, w);
    }
    format!("{}{s}", " ".repeat(w - cw))
}

// ---------------------------------------------------------------- cabecera

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let style = Style::new()
        .fg(app.pal.bright)
        .bg(app.pal.accent)
        .add_modifier(Modifier::BOLD);
    let left = format!(" 󰕮 QUICKSHELL  ·  {} ", t("app.subtitle"));
    let mut parts: Vec<String> = vec![];
    if app.store.dirty() {
        let n = app.store.changes().len() + app.store.ops.len();
        parts.push(format!(
            "*{}*",
            tf("hdr.pending", &[("n", &n.to_string())]).to_uppercase()
        ));
    }
    if app.store.paths.sandbox {
        parts.push("sandbox".into());
    } else if !app.shell_running {
        parts.push(t("hdr.shell_off"));
    }
    parts.push(if app.advanced {
        t("hdr.advanced")
    } else {
        t("hdr.simple")
    });
    parts.push(i18n::lang().code().to_uppercase());
    parts.push(format!("q: {}", t("ft.quit")));
    let mut right = format!(" {} ", parts.join(" │ "));
    let w = area.width as usize;
    let left = if left.width() + right.width() > w {
        " 󰕮 QUICKSHELL ".to_string()
    } else {
        left
    };
    if left.width() + right.width() > w {
        right = String::new();
    }
    let space = w.saturating_sub(left.width() + right.width());
    let line = format!("{left}{}{right}", " ".repeat(space));
    f.render_widget(Line::from(Span::styled(truncate(&line, w), style)), area);
}

// ---------------------------------------------------------------- sidebar

/// Categorías del sidebar: (clave de texto, secciones).
const CATEGORIES: [(&str, &[Section]); 3] = [
    ("cat.bar", &[Section::Bar, Section::Widgets]),
    (
        "cat.system",
        &[Section::Plugins, Section::Idle, Section::Appearance],
    ),
    ("cat.review", &[Section::Changes]),
];

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let w = area.width as usize;
    let active = app.focus == Focus::Sidebar && app.popup.is_none();
    let mut y = area.y;
    for (n, (cat, sections)) in CATEGORIES.iter().enumerate() {
        if n > 0 {
            y += 1;
        }
        if y >= area.bottom() {
            break;
        }
        put(
            f,
            area.x,
            y,
            vec![Span::styled(
                pad(&format!(" {}", t(cat).to_uppercase()), w),
                fg(app.pal.muted).add_modifier(Modifier::BOLD),
            )],
        );
        y += 1;
        for s in sections.iter() {
            if y >= area.bottom() {
                break;
            }
            let i = s.index();
            let hit = Hit::Sidebar(i);
            let hover = hovered(app, hit);
            let selected = *s == app.section;
            let mut title = s.title();
            if *s == Section::Changes && app.store.dirty() {
                title.push_str(&format!(
                    " ({})",
                    app.store.changes().len() + app.store.ops.len()
                ));
            }
            let prefix = if hover { " ▸" } else { "  " };
            let text = pad(&format!("{prefix}{}  {title}", s.icon()), w);
            let strong = |bg: Color| {
                Style::new()
                    .fg(app.pal.bright)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            };
            let style = if selected && (active || hover) {
                strong(app.pal.soft_selection)
            } else if selected || hover {
                strong(app.pal.soft_muted)
            } else {
                fg(app.pal.fg)
            };
            put(f, area.x, y, vec![Span::styled(text, style)]);
            app.hits.push((Rect::new(area.x, y, area.width, 1), hit));
            y += 1;
        }
    }

    // Interruptores de modo e idioma al pie del sidebar, también clicables.
    let bottom = area.bottom();
    if bottom < y + 4 {
        return;
    }
    put(
        f,
        area.x,
        bottom - 3,
        vec![Span::styled(
            format!(" {}", "─".repeat(w.saturating_sub(2))),
            fg(app.pal.muted),
        )],
    );
    let toggles = [
        (
            Hit::ModeToggle,
            "󰘵",
            t("side.mode"),
            if app.advanced {
                t("hdr.advanced")
            } else {
                t("hdr.simple")
            },
        ),
        (
            Hit::LangToggle,
            "󰗊",
            t("side.lang"),
            if i18n::lang() == i18n::Lang::Es {
                "Español".to_string()
            } else {
                "English".to_string()
            },
        ),
    ];
    for (n, (hit, icon, label, value)) in toggles.into_iter().enumerate() {
        let y = bottom - 2 + n as u16;
        let hover = hovered(app, hit);
        let lead = if hover { " ▸" } else { "  " };
        let right = format!("{value} ⇄ ");
        // Sin espacio suficiente solo se muestra el icono.
        let left = if format!("{lead}{icon}  {label} {right}").width() <= w {
            format!("{lead}{icon}  {label}")
        } else {
            format!("{lead}{icon}")
        };
        let style = if hover {
            Style::new()
                .fg(app.pal.bright)
                .bg(app.pal.soft_hover)
                .add_modifier(Modifier::BOLD)
        } else {
            fg(app.pal.fg)
        };
        let val_style = if hover {
            style
        } else {
            fg(app.pal.accent).add_modifier(Modifier::BOLD)
        };
        let gap = w.saturating_sub(left.width() + right.width());
        put(
            f,
            area.x,
            y,
            vec![
                Span::styled(format!("{left}{}", " ".repeat(gap)), style),
                Span::styled(right, val_style),
            ],
        );
        app.hits.push((Rect::new(area.x, y, area.width, 1), hit));
    }
}

// ---------------------------------------------------------------- pie

/// Atajo del pie: (tecla, descripción, prioridad). Con poco ancho se ocultan
/// primero los de prioridad más baja.
type Hint = (String, String, u8);

fn footer_hints(app: &App) -> Vec<Hint> {
    let k = |a: &str, b: &str, p: u8| (a.to_string(), t(b), p);
    if let Some(p) = &app.popup {
        return match p {
            Popup::Confirm { .. } => vec![k("Enter", "ft.yes", 3), k("Esc", "ft.no", 3)],
            Popup::Message { .. } => vec![k("Enter", "ft.close", 3)],
            Popup::Help { .. } => vec![k("↑↓", "ft.scroll", 2), k("Esc", "ft.close", 3)],
            Popup::Conflict { .. } => vec![
                k("o", "ft.overwrite", 3),
                k("r", "ft.reload", 3),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Input(_) => vec![
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
                k("Ctrl+U", "ft.clear", 1),
            ],
            Popup::Picker(_) => vec![
                k("↑↓", "ft.move", 1),
                k("Enter", "ft.choose", 3),
                k("abc", "ft.filter", 2),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Checklist(_) => vec![
                k(&t("key.space"), "ft.check", 3),
                k("a", "ft.all", 1),
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Menu(_) => vec![
                k("↑↓", "ft.move", 2),
                k("Enter", "ft.choose", 3),
                k("Esc", "ft.close", 3),
            ],
        };
    }
    let mut v = vec![];
    match (app.focus, app.section) {
        (Focus::Sidebar, _) => {
            v.push(k("↑↓", "ft.section", 1));
            v.push(k("Enter", "ft.open", 2));
        }
        (Focus::Buttons, _) => {
            v.push(k("←→", "ft.choose_button", 2));
            v.push(k("Enter", "ft.press", 3));
        }
        (Focus::Content, Section::Widgets) if app.widgets.editing.is_none() => {
            v.push(k("⇧+←→↑↓", "ft.reorder", 1));
            v.push(k("Enter", "ft.settings", 2));
            v.push(k("n", "ft.add", 2));
            v.push(k("d", "ft.remove", 2));
        }
        (Focus::Content, Section::Plugins) => {
            v.push(k("Enter", "ft.toggle", 2));
            v.push(k("/", "ft.filter", 1));
        }
        _ => {
            v.push(k("←→", "ft.change", 2));
            v.push(k("Enter", "ft.edit", 2));
            v.push(k("r", "ft.reset", 1));
        }
    }
    v.push(k("o", "ft.menu", 1));
    v.push(k("Tab", "ft.next_area", 0));
    v.push(k("?", "ft.help", 3));
    v
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let base = Style::new().fg(app.pal.bright).bg(app.pal.soft_muted);
    // Izquierda: aviso reciente, o la explicación de lo que hay bajo el ratón.
    let status = match &app.toast {
        Some(toast) => {
            let icon = match toast.kind {
                NoteKind::Info => "",
                NoteKind::Warn => "",
            };
            format!(" {icon} {}", toast.text)
        }
        None => app
            .hover_hint()
            .map(|h| format!(" 󰳽 {h}"))
            .unwrap_or_default(),
    };
    let status_style = if app.toast.is_some() {
        base.add_modifier(Modifier::BOLD)
    } else {
        base
    };

    let hints = footer_hints(app);
    let render = |h: &[&Hint]| -> String {
        if h.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = h.iter().map(|(k, d, _)| format!("{k}: {d}")).collect();
        format!(" {} ", parts.join(" │ "))
    };
    let mut keep: Vec<&Hint> = hints.iter().collect();
    let width = area.width as usize;
    let budget = width.saturating_sub(status.width().min(width / 2) + 1);
    for prio in 0..4u8 {
        while render(&keep).width() > budget {
            match keep.iter().position(|h| h.2 == prio) {
                Some(i) => {
                    keep.remove(i);
                }
                None => break,
            }
        }
    }
    let right = render(&keep);
    let left_w = width.saturating_sub(right.width());
    f.render_widget(
        Line::from(vec![
            Span::styled(pad(&status, left_w), status_style),
            Span::styled(right, base),
        ]),
        area,
    );
}
