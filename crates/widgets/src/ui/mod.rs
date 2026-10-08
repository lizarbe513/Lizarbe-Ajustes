//! Dibujo de la interfaz, con el estilo visual de Meca: barra de título en
//! color de acento, sidebar por categorías, filas de tres líneas con controles
//! en relieve, botones 3D y barra de estado inferior. Todo elemento clicable
//! reacciona al pasar el ratón.

pub mod form;
pub mod popup;
mod views;

use lizarbe_core::ui::{Hint, rule_h, rule_v, status_bar};
pub(crate) use lizarbe_core::ui::{fg, pad, put, truncate, wrap};
use lizarbe_core::view::{self, Ctx, SideItem, SideToggle};
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
    let [header, _gap, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, header);
    let side_w = if area.width < 95 { 22 } else { 28 };
    let [side, sep, content] = Layout::horizontal([
        Constraint::Length(side_w),
        Constraint::Length(1),
        Constraint::Min(20),
    ])
    .areas(body);
    draw_sidebar(f, app, side);
    let rule = app.pal.rule;
    // Una línea vertical entre el menú y el contenido; la horizontal de los
    // botones se cruza con ella en `┴`.
    let rule_y = body.bottom().saturating_sub(views::BUTTONS_H);
    rule_v(f, sep.x, sep.y, rule_y.saturating_sub(sep.y), rule);
    rule_h(f, area.x + 2, rule_y, area.width.saturating_sub(4), rule);
    put(f, sep.x, rule_y, vec![Span::styled("┴", fg(rule))]);
    views::draw_content(f, app, content, rule_y);
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

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let mut parts: Vec<String> = vec![];
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
    let n = app.store.changes().len() + app.store.ops.len();
    let pending =
        (app.store.dirty() && n > 0).then(|| tf("status.pending", &[("n", &n.to_string())]));
    let name = t("app.name");
    let subtitle = t("app.subtitle");
    let close = t("btn.close");
    let back_label = t("btn.back");
    // Esc retrocede mientras el foco no esté ya en el menú de la izquierda.
    let search = app.popup.is_none().then(|| ("/", t("btn.search")));
    let back = (app.focus != Focus::Sidebar && app.popup.is_none())
        .then_some(("Esc", back_label.as_str()));
    view::draw_header(
        f,
        &mut ctx(app),
        area,
        &name,
        &subtitle,
        &parts,
        pending.as_deref(),
        ("Q", &close),
        back,
        search.as_ref().map(|(k, l)| (*k, l.as_str())),
    );
}

// ---------------------------------------------------------------- sidebar

/// Categorías del sidebar: (clave de texto, secciones).
const CATEGORIES: [(&str, &[Section]); 3] = [
    ("cat.bar", &[Section::Bar, Section::Widgets]),
    (
        "cat.system",
        &[
            Section::Plugins,
            Section::Idle,
            Section::Notifications,
            Section::Appearance,
        ],
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
            Popup::Color(_) => vec![
                k("←→", "ft.change", 3),
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
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
            v.push(k("Esc", "ft.close", 3));
        }
        (Focus::Buttons, _) => {
            v.push(k("←→", "ft.choose_button", 2));
            v.push(k("Enter", "ft.press", 3));
            v.push(k("Esc", "ft.back", 3));
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
    if app.focus == Focus::Content {
        v.push(k("Esc", "ft.back", 3));
    }
    if !(app.focus == Focus::Content && app.section == Section::Plugins) {
        v.push(k("/", "ft.search", 2));
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
                NoteKind::Info => "✓",
                NoteKind::Warn => "▲",
            };
            format!("{icon} {}", toast.text)
        }
        None => app.hover_hint().unwrap_or_default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omarchy::paths::Paths;
    use crate::store::Store;
    use lizarbe_core::prefs::Prefs;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        i18n::set_lang(i18n::Lang::Es);
        (dir, App::new(store, Prefs::default()))
    }

    fn render(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    #[test]
    fn every_section_draws_at_every_width() {
        let (_d, mut app) = app();
        for w in [64u16, 80, 120] {
            for s in Section::ALL {
                app.section = s;
                let lines = render(&mut app, w, 36);
                let text = lines.join("\n");
                assert!(text.contains("[Q] Cerrar"), "{s:?} a {w}: falta cerrar");
                assert!(lines.iter().all(|l| l.chars().count() <= w as usize));
            }
        }
    }

    #[test]
    fn footer_buttons_show_their_keys() {
        let (_d, mut app) = app();
        let text = render(&mut app, 120, 36).join("\n");
        for needle in ["[A] Aplicar", "[C] Cancelar", "[R] Restaurar"] {
            assert!(text.contains(needle), "falta {needle}");
        }
    }

    #[test]
    fn search_button_is_in_the_header_and_jumps_to_an_option() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let (_d, mut app) = app();
        let text = render(&mut app, 120, 36).join("\n");
        assert!(text.contains("[/] Buscar"), "{text}");
        app.section = Section::Idle;
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(matches!(app.popup, Some(Popup::Picker(_))));
        let first = app
            .search_entries()
            .into_iter()
            .find(|e| e.section == Section::Appearance.index() && !e.key.is_empty())
            .unwrap();
        for c in first.label.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.popup.is_none());
        assert_eq!(app.section, Section::Appearance);
        // en Plugins, `/` sigue filtrando plugins
        app.go_section(Section::Plugins);
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(app.popup.is_none());
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(matches!(app.popup, Some(Popup::Picker(_))));
    }

    #[test]
    fn notifications_section_draws_and_is_reachable_by_id() {
        let (_d, mut app) = app();
        assert_eq!(
            Section::from_id("notificaciones"),
            Some(Section::Notifications)
        );
        app.go_section(Section::Notifications);
        let text = render(&mut app, 120, 36).join("\n");
        for needle in [
            "NOTIFICACIONES",
            "No molestar",
            "Dejar que swaync muestre avisos",
        ] {
            assert!(text.contains(needle), "falta {needle}\n{text}");
        }
    }
}
