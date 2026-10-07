//! Dibujo de Escritorio con las piezas compartidas del núcleo: barra de
//! título, barra lateral, formularios, botones y barra de estado.

use lizarbe_core::form::NoteKind;
use lizarbe_core::ui::{Hint, rule_h, rule_v, status_bar};
use lizarbe_core::view::{self, ButtonSpec, Ctx, FormOpts, RowInfo, SideItem, SideToggle};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};

use crate::app::{App, Button, FieldRow, Focus, Hit, Popup, Section};
use crate::catalog::CATEGORIES;
use crate::i18n::{self, t, tf};

/// Alto de la zona de botones: la línea fina, los botones y una línea de aire.
const BUTTONS_H: u16 = 3;

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
    let rule_y = body.bottom().saturating_sub(BUTTONS_H);
    rule_v(f, sep.x, sep.y, rule_y.saturating_sub(sep.y), rule);
    rule_h(f, area.x + 2, rule_y, area.width.saturating_sub(4), rule);
    lizarbe_core::ui::put(
        f,
        sep.x,
        rule_y,
        vec![ratatui::text::Span::styled("┴", lizarbe_core::ui::fg(rule))],
    );
    draw_content(f, app, content, rule_y);
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

fn ctx(app: &mut App) -> Ctx<'_, Hit> {
    Ctx {
        pal: &app.pal,
        hover: app.hover,
        hits: &mut app.hits,
        blocked: app.popup.is_some(),
    }
}

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let mut parts: Vec<String> = vec![];
    if app.store.paths.sandbox {
        parts.push("sandbox".into());
    } else if !app.store.live {
        parts.push(t("hdr.offline"));
    }
    parts.push(if app.advanced {
        t("hdr.advanced")
    } else {
        t("hdr.simple")
    });
    parts.push(i18n::lang().code().to_uppercase());
    let pending = app.pending();
    let pending_text = (app.store.dirty() && pending > 0)
        .then(|| tf("status.pending", &[("n", &pending.to_string())]));
    let name = t("app.name");
    let subtitle = t("app.subtitle");
    let close = ("Q", t("btn.close"));
    let back_label = t("btn.back");
    // Esc retrocede mientras el foco no esté ya en el menú de la izquierda.
    let back = (app.focus != Focus::Sidebar && app.popup.is_none())
        .then_some(("Esc", back_label.as_str()));
    view::draw_header(
        f,
        &mut ctx(app),
        area,
        &name,
        &subtitle,
        &parts,
        pending_text.as_deref(),
        (close.0, &close.1),
        back,
    );
}

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Sidebar && app.popup.is_none();
    let pending = app.pending();
    let categories: Vec<(String, Vec<SideItem>)> = CATEGORIES
        .iter()
        .map(|(cat, sections)| {
            let items = sections
                .iter()
                .map(|s| {
                    let mut title = s.title();
                    if *s == Section::Changes && pending > 0 {
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
    view::draw_sidebar(f, &mut ctx(app), area, &categories, &toggles, active);
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect, rule_y: u16) {
    let focused = app.focus == Focus::Content && app.popup.is_none();
    let x = area.x;
    let w = area.width;
    if area.height < 9 || w < 30 {
        return;
    }
    let head_h = view::section_header(
        f,
        &app.pal,
        Rect::new(x + 1, area.y + 1, w.saturating_sub(2), 4),
        &app.section.title(),
        &app.section.description(),
        focused,
    );

    let top = area.y + 1 + head_h;
    let body = Rect::new(x, top, w, rule_y.saturating_sub(top));
    draw_form(f, app, body, focused);
    draw_buttons(f, app, Rect::new(x, rule_y + 1, w, 1));
}

fn draw_form(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let rows = app.rows();
    let opts = FormOpts {
        focused,
        dropdown_open: matches!(&app.popup, Some(Popup::Picker(p)) if p.anchor.is_some()),
    };
    let advanced = app.advanced;
    let App {
        store,
        pal,
        hover,
        hits,
        popup,
        forms,
        section,
        ..
    } = app;
    let st = &mut forms[section.index()];
    let mut ctx = Ctx {
        pal,
        hover: *hover,
        hits,
        blocked: popup.is_some(),
    };
    let info = |fr: &FieldRow| RowInfo {
        changed: lizarbe_core::form::FieldStore::get_original(store, &fr.bind) != fr.value,
        extra: advanced.then(|| {
            format!(
                "{}: {} · {}: {}",
                t("help.key"),
                fr.def.key,
                t("help.default"),
                fr.def
                    .default
                    .as_ref()
                    .map(lizarbe_core::schema::value_label)
                    .unwrap_or_else(|| "—".into())
            )
        }),
    };
    view::draw_form(f, &mut ctx, area, &rows, st, &opts, info);
}

fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let buttons: Vec<ButtonSpec> = Button::ALL
        .iter()
        .map(|b| ButtonSpec {
            label: b.label(),
            key: b.key().to_string(),
            enabled: app.button_enabled(*b),
            primary: *b == Button::Apply,
        })
        .collect();
    let pending = app.pending();
    let (status, color) = if pending > 0 {
        (
            format!("• {}", tf("btn.pending", &[("n", &pending.to_string())])),
            app.pal.accent,
        )
    } else {
        (format!("✓ {}", t("btn.clean")), app.pal.ok)
    };
    let selected = focused.then_some(app.button);
    view::draw_buttons(f, &mut ctx(app), area, &buttons, selected, (&status, color));
}

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
    let mut v = match app.focus {
        Focus::Sidebar => vec![
            k("↑↓", "ft.section", 1),
            k("Enter", "ft.open", 2),
            k("Esc", "ft.close", 3),
        ],
        Focus::Buttons => vec![
            k("←→", "ft.choose_button", 2),
            k("Enter", "ft.press", 3),
            k("Esc", "ft.back", 3),
        ],
        Focus::Content => vec![
            k("←→", "ft.change", 2),
            k("Enter", "ft.edit", 2),
            k("r", "ft.reset", 1),
            k("Esc", "ft.back", 3),
        ],
    };
    v.push(k("o", "ft.menu", 1));
    v.push(k("Tab", "ft.next_area", 0));
    v.push(k("?", "ft.help", 3));
    v
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
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
    use crate::paths::Paths;
    use crate::store::Store;
    use lizarbe_core::prefs::Prefs;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hyprland.lua"),
            "require(\"hypr.looknfeel\")\n",
        )
        .unwrap();
        let store = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        i18n::set_lang(i18n::Lang::Es);
        (dir, App::new(store, Prefs::default()))
    }

    /// Dibuja la aplicación y devuelve cada línea de la pantalla como texto.
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
                app.go_section(s);
                let lines = render(&mut app, w, 36);
                assert!(
                    lines.iter().any(|l| l.contains("Cerrar")),
                    "{s:?} a {w} columnas: falta el botón de cerrar"
                );
            }
        }
    }

    #[test]
    fn footer_buttons_show_their_keys() {
        let (_d, mut app) = app();
        let lines = render(&mut app, 120, 36);
        let text = lines.join("\n");
        for needle in ["[A] Aplicar", "[C] Cancelar", "[R] Restaurar", "[Q] Cerrar"] {
            assert!(text.contains(needle), "falta {needle}\n{text}");
        }
    }

    #[test]
    fn rule_lines_cross_in_a_junction() {
        let (_d, mut app) = app();
        let lines = render(&mut app, 120, 36);
        let row = lines
            .iter()
            .position(|l| l.contains('┴'))
            .expect("falta el cruce ┴");
        let col = lines[row].chars().position(|c| c == '┴').unwrap();
        // La línea vertical llega justo hasta el cruce.
        assert_eq!(lines[row - 1].chars().nth(col), Some('│'));
    }

    #[test]
    fn nothing_overflows_a_narrow_window() {
        let (_d, mut app) = app();
        app.go_section(Section::Keybinds);
        let lines = render(&mut app, 64, 30);
        assert!(lines.iter().all(|l| l.chars().count() <= 64));
    }

    #[test]
    fn esc_hint_follows_the_focus() {
        let (_d, mut app) = app();
        app.focus = crate::app::Focus::Content;
        let text = render(&mut app, 120, 36).join("\n");
        assert!(text.contains("[Esc] Volver"), "{text}");
        app.focus = crate::app::Focus::Sidebar;
        let text = render(&mut app, 120, 36).join("\n");
        assert!(!text.contains("[Esc] Volver"));
    }

    #[test]
    fn sidebar_shows_section_icons() {
        let (_d, mut app) = app();
        let text = render(&mut app, 120, 36).join("\n");
        assert!(text.contains(crate::app::Section::Monitors.icon()));
    }

    /// `cargo test dump_screens -- --ignored --nocapture` imprime las pantallas.
    #[test]
    #[ignore = "solo para revisar el aspecto a mano"]
    fn dump_screens() {
        let (_d, mut app) = app();
        for s in Section::ALL {
            app.go_section(s);
            println!("\n===== {s:?} =====");
            for l in render(&mut app, 100, 34) {
                println!("{}", l.trim_end());
            }
        }
    }
}
