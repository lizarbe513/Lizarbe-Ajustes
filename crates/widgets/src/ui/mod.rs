//! Dibujo de la interfaz, con el estilo visual de Meca: barra de título en
//! color de acento, sidebar por categorías, filas de tres líneas con controles
//! en relieve, botones 3D y barra de estado inferior. Todo elemento clicable
//! reacciona al pasar el ratón.

pub mod form;
pub mod popup;
mod views;

use lizarbe_core::ui::{Hint, header_bar, status_bar};
pub(crate) use lizarbe_core::ui::{fg, pad, put, truncate, wrap};
use lizarbe_core::view::{Ctx, SideItem, SideToggle};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Span;

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

    if let Some(popup) = app.popup.as_mut() {
        let mut ctx = Ctx {
            pal: &app.pal,
            hover: app.hover,
            hits: &mut app.hits,
            blocked: false,
        };
        lizarbe_core::modal::draw(f, &mut ctx, area, popup);
    }
}

/// Contexto de dibujo para las piezas del núcleo.
pub(crate) fn ctx(app: &mut App) -> Ctx<'_, Hit> {
    Ctx {
        pal: &app.pal,
        hover: app.hover,
        hits: &mut app.hits,
        blocked: app.popup.is_some(),
    }
}

/// ¿Está el ratón sobre este elemento (y no hay ventana encima)?
pub(crate) fn hovered(app: &App, hit: Hit) -> bool {
    app.popup.is_none() && app.hover == Some(hit)
}

// ---------------------------------------------------------------- cabecera

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let left = format!(" 󰕮 {}  ·  {} ", t("app.name").to_uppercase(), t("app.subtitle"));
    let left_short = format!(" 󰕮 {} ", t("app.name").to_uppercase());
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
    header_bar(f, &app.pal, area, &left, &left_short, &parts);
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
    let active = app.focus == Focus::Sidebar && app.popup.is_none();
    let pending = app.store.changes().len() + app.store.ops.len();
    let categories: Vec<(String, Vec<SideItem>)> = CATEGORIES
        .iter()
        .map(|(cat, sections)| {
            let items = sections
                .iter()
                .map(|s| {
                    let mut title = s.title();
                    if *s == Section::Changes && app.store.dirty() {
                        title.push_str(&format!(" ({pending})"));
                    }
                    SideItem {
                        icon: s.icon(),
                        title,
                        selected: *s == app.section,
                    }
                })
                .collect();
            (t(cat), items)
        })
        .collect();
    let toggles = [
        SideToggle {
            hit: Hit::ModeToggle,
            icon: "󰘵",
            label: t("side.mode"),
            value: if app.advanced {
                t("hdr.advanced")
            } else {
                t("hdr.simple")
            },
        },
        SideToggle {
            hit: Hit::LangToggle,
            icon: "󰗊",
            label: t("side.lang"),
            value: if i18n::lang() == i18n::Lang::Es {
                "Español".to_string()
            } else {
                "English".to_string()
            },
        },
    ];
    lizarbe_core::view::draw_sidebar(f, &mut ctx(app), area, &categories, &toggles, active);
}

// ---------------------------------------------------------------- pie

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
    status_bar(
        f,
        &app.pal,
        area,
        &status,
        app.toast.is_some(),
        &footer_hints(app),
    );
}
